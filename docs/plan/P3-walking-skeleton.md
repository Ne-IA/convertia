# P3 — Walking Skeleton (first conversion end-to-end)

> **One trivial conversion, fully through the real architecture, on all 3 OS.** P3
> drives the **in-core CSV→TSV** path (`EngineProgram::InProcessNative`, §3.5.6 — no
> sidecar binary) end-to-end: **drop → detect → pick target → convert → atomic,
> no-overwrite publish → result UI**. It proves the Tauri + atomic-publish + IPC stack
> early, before any heavy engine, so P5–P7 broaden coverage on a proven harness instead
> of discovering architectural problems late (README "walking skeleton first").
>
> Derives from [01-conversion-pipeline](../spec/01-conversion-pipeline.md) (§1.2 layered
> detection, §1.7 dispatch incl. the `InProcessNative` sub-case, §1.8–§1.12 the convert
> spine), [02-guarantees](../spec/02-guarantees.md) (§2.0–§2.7 the `crate::fs_guard`
> no-clobber atomic-publish kernel incl. §2.3 link-safety [T7], §2.4 frozen set [T8],
> §2.5 re-run detection, §2.6 cleanup/temp-ownership, §2.7 destination/divert incl. the
> §2.1.2/§2.7.2 FAT/exFAT divert), [03-engines-and-bundling](../spec/03-engines-and-bundling.md)
> (§3.5.6 native CSV/TSV engine, §3.2 the `Engine`/`Invocation`/`EngineProgram` seam),
> and [05-ui-ux](../spec/05-ui-ux.md) (the minimal walking-skeleton screen path).
> Index: [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **This is the v0 base** — the smallest atomic `[ ]` boxes below; a later adversarial
> review deepens/splits/completes them. Built by the loop (P1..P11 range) strictly top to
> bottom, deps resolved in place (DECISION C).
>
> **What P3 builds vs defers (the boundary, decided in plan/README.md):**
> - **`crate::fs_guard` is BUILT HERE** (the real atomic-publish OS primitives + FAT/exFAT
>   divert) — it has **no engine dependency** and is exercised by the walking-skeleton
>   publish; P4's §02 reference covers only the §2.12 isolation wrapper + §2.13 app-fault,
>   **not** a re-implementation of `fs_guard` (P4 must not rebuild it).
> - **`crate::isolation` + the §0.9 pool are compile-time SHELLS here** — the §1.7 dispatch
>   enum reaches the `InProcessNative` branch only; if its types reference `crate::isolation`
>   / the pool envelope, P3 creates **interface-only shells so the InProcessNative path
>   compiles without spawning anything**. At runtime CSV→TSV genuinely bypasses
>   isolation/pool (§2.12.4 absolute: pure in-core memory-safe Rust, no subprocess), so this
>   is a compile-time stub, **not** a forward dependency on P4. **P4 fills these shells**
>   (it does not build them from scratch).
> - **No image-worker / libvips / any subprocess engine here** — the first real sidecar is
>   validated in P4/P5.
>
> **Activation targets for P0 (named box-ids):** several P0 boxes carry `→ activated in P3` /
> `→ activated in P3+` notes — the concrete P3 boxes they activate against are:
> G31/G32 output-validity (P0.5.5) → **P3.38/P3.62**; the §1.2 detection-fuzz KAT
> (P0.5.7) → **P3.30** (the KAT [TEST] box, `needs: P3.29`); the detect/`fs_guard` fuzz targets + the replay convention
> (P0.4.3 / P0.5.8) → **P3.29** (`crate::detect` body, P3.26–P3.29; P3.30 is the KAT landmark)**/P3.6/P3.8/P3.41 + P3.67**; the §2.1.3
> atomicity-under-interruption home (P0.5.9) → **P3.19.1**, the §2.1.2 Windows-AV-retry
> fault-injection home (P0.5.9) → **P3.19.2** (the two split into separately-faileable sub-boxes
> under the P3.19 grouping shell); the §2.14.1 temp-ownership / lock-before-part MECHANISM (P0.5.9) →
> **P3.20/P3.21** with its dedicated security-TEST home → **P3.71**; the T7/T8
> link-safety/self-feeding homes (P0.5.9) → **P3.64**;
> `cargo-mutants` over the `fs_guard`/`detect`/`outcome` kernel (P0.5.10) → the runnable
> first-informational-pass [GATE] box **P3.72** (`needs: P0.5.10` + the **P3.6/P3.8/P3.18 /
> P3.29 / P3.46 / P3.68/P3.69** kernel-body boxes it injects mutants into; the §2.8.2/§2.9.1
> catalogs are the `crate::outcome` string-table leg) — the box that RUNS the gate +
> initialises the per-crate ratchet + the `gate-status.md` entry, mirroring P3.67's
> fuzz-replay activation. Both P3.67 (`needs: P0.5.8`) and P3.72 (`needs: P0.5.10`) carry the
> explicit P0→P3 edge; the other activation edges resolve trivially since P0 is `[x]` before
> the loop reaches P3.

## Boundaries (decided, see plan/README.md)

- **P2 → P3:** P2 declared the pipeline **contracts + domain types** (§0.6 `DroppedItem`/
  `CollectedSet`/`OutputPlan`/`RunResult`/`JobStage`/`JobState`, the §0.4 IPC
  command/event signatures, the §1.1 intake state machine, the C12 `EngineHealth`
  contract). **P3 IMPLEMENTS** the bodies behind them for the CSV→TSV slice. A box that
  needs a P2-declared type/handler-stub carries an explicit `needs: P2.<n>` edge; the
  load-bearing P3→P2 edges are wired on the boxes (the type-consuming ones) and owned by
  the P3.70 reconciliation box so no P3→P2 dependency is left implicit.
- **P3 → P4:** P3 establishes the `crate::isolation` + pool **interface shells**; P4
  **expands** them into the real §2.12 wrapper + §0.9 pool. P3 builds the **real**
  `crate::fs_guard`.
- **No-clobber/atomic kernel is content-independent:** every `fs_guard` box is fully
  buildable now (a real temp FS + a real publish primitive), independent of any engine.

---

### Module skeletons & the InProcessNative dispatch seam

**Goal:** the Rust modules the walking skeleton lives in exist and the §1.7 dispatch
reaches the `InProcessNative` branch — with `crate::isolation` and the §0.9 pool present as
**compile-time interface shells** so the in-core path compiles without spawning anything.

- [ ] **P3.1** [RUST] Scaffold the `crate::fs_guard` / `crate::run` / `crate::outcome` module roots with their public surface · §2.0 §0.7
  > stand up the three §2.0 trust-kernel leaf-module roots (no dependency on UI/IPC/engine-registry; the layer never calls back up, §2.0 dependency direction) as three INDEPENDENT sub-boxes — they have no mutual dependency at scaffold time, so each is worked + checked off on its own (the loop works them top-to-bottom; a body issue in one root does not couple to the others). `#![deny(unsafe_code)]` at the crate root (no FFI in any P3 module). Downstream boxes that consume one root `needs: P3.1` (the parent gates on all three roots existing — each downstream box needs only the root it names in its own body).
  - [ ] **P3.1.1** [RUST] Scaffold the `crate::fs_guard` module root + its public function shells · §2.0 §0.7
    > `src-tauri/src/fs_guard/` with public shells only (`atomic_publish`, `output_name`, `resolve_identity`, `is_safe_output`, `check_path_limit`, `location_status`) wired so the later `fs_guard` boxes (P3.6–P3.19, P3.31–P3.40) fill the bodies.
  - [ ] **P3.1.2** [RUST] Scaffold the `crate::run` module root + its public function shells · §2.0 §0.7
    > `src-tauri/src/run/` with public shells only (`cleanup_item`/`cleanup_run`/`sweep_stale` + the run-lifecycle ordering seam) wired so the later `run` boxes (P3.20–P3.25) fill the bodies; no dependency on `fs_guard` to compile its root.
  - [ ] **P3.1.3** [RUST] Scaffold the `crate::outcome` error-taxonomy module root · §2.0 §0.7 §2.8
    > `src-tauri/src/outcome/` as the §2.8 taxonomy + `OutcomeMsg`/`CleanupResidue` string home (the `From<ConversionErrorKind>` projection seam P3.46 fills; the §2.8.2 message catalog P3.68 + the §2.9.1 lossy-note catalog P3.69 author the string TABLES) wired so the later boxes fill the bodies; no dependency on `fs_guard`/`run` to compile its root.
- [ ] **P3.2** [RUST] Build the `crate::isolation` compile-time interface shell (no spawn, no FFI) · §2.12.1 §2.12.4 §0.7
  needs: P3.1
  > the §2.12 decoder-isolation wrapper as an **interface-only shell** so the §1.7 dispatch enum's `Subprocess` arm type-checks; it spawns nothing in P3. CSV→TSV genuinely bypasses it at runtime (§2.12.4 absolute: pure in-core memory-safe Rust, no third-party C/C++ decoder), so this is a compile-time stub, NOT a forward dependency on P4. P4 expands this shell into the real wrapper (it must not rebuild it from scratch).
- [ ] **P3.3** [RUST] Build the §0.9 subprocess-pool interface shell + the in-core worker-thread permit lane · §0.9 §1.7
  needs: P3.1
  > the §0.9 pool as an interface shell carrying the **global-degree** permit model; expose the **`spawn_blocking`-style worker-thread lane** the `InProcessNative` engine actually uses (it holds a global-degree permit, has **no** `serialised_only` lane, runs on dedicated worker threads so the synchronous CSV loop never blocks the Tokio runtime, §0.9 native-CSV/TSV row / §1.7 concurrency-permit model). The subprocess permit/semaphore machinery is shelled (P4 fills it); only the in-core lane is live in P3.
- [ ] **P3.4** [RUST] Wire the §1.7 dispatch enum reaching the `InProcessNative` branch only · §1.7 §3.2 · G29
  needs: P3.2, P3.3
  > the §1.7 `EngineInvocation` dispatch envelope `(JobId, EngineId, Invocation, CancellationToken)` and the match on `Invocation.program: EngineProgram` (§3.2). In P3 only the **`InProcessNative(EngineId)`** arm is implemented; the `Subprocess`/`ResourceBin` arms compile against the P3.2 isolation shell but are unreachable in the walking skeleton. Exhaustive-match deny on `EngineProgram` (no `_ =>` catch-all, the §1.2/G29 deny set, P0.4.1) so a future engine arm cannot be silently dropped.
- [ ] **P3.5** [RUST] Define the `Engine` trait `plan()` for the native CSV/TSV engine (Pure, no I/O) · §3.2 §3.5.6
  needs: P3.4
  > the §3.2 `Engine::plan(&job) -> Result<Invocation, PlanError>` impl for the native engine: returns an `Invocation { program: InProcessNative(EngineId::NativeCsvTsv), out_tmp: Some(<dest-dir publish temp>), progress: ProgressModel::InProcessFraction, .. }` (§3.2.2). `plan()` is **Pure** (no I/O, no spawn); single-step engine so `plan_encode` is never reached (the default-impl InternalError, §3.2). `EngineId::NativeCsvTsv` is absent from the §3.3.3 `EngineId → binary-name` table (no sidecar to resolve, §3.5.6).

---

### `crate::fs_guard`: resolved-identity, link-safety & path-limit (the no-harm kernel)

**Goal:** the load-bearing "same file?" / "would this write touch an original?" / "would
this name overflow the path limit?" predicates exist on all 3 OS — the §2.3/§2.2.3 half of
`fs_guard` the atomic publish sits on top of. These are the BRANCH-coverage-floored
kernel functions (P0.4.8) and `cargo-mutants` targets (P0.5.10).

- [ ] **P3.6** [RUST] Build `fs_guard::resolve_identity` — canonical path + OS file identity, per OS · §2.3.1 §2.3.4 · G9
  needs: P3.1
  > `resolve_identity(path) -> FileIdentity { canonical_path, dev_or_volserial, inode_or_fileindex }`: `std::fs::canonicalize` primary; **Unix** `(st_dev, st_ino)` via `MetadataExt`; **Windows** `(volumeSerialNumber, fileIndexHigh, fileIndexLow)` via `GetFileInformationByHandle` (catches hardlinks + junctions `canonicalize` misses, §2.3.4); **macOS** `(st_dev, st_ino)` + Finder-alias-is-a-data-file note. Display/comparison form normalised via `dunce::canonicalize` so a `\\?\`-prefix difference compares equal (§2.3.1). G9 invariant (d) no raw `127.0.0.1`/`localhost` — N/A here but the module joins the grep scope.
- [ ] **P3.7** [RUST] Build the resolved-identity de-dup of the frozen set · §2.3.2 §2.4.1
  needs: P3.6
  > key every frozen entry by `FileIdentity`; two dropped paths resolving to the same inode/file-index collapse to **one** `DroppedItem` (converted once, SSOT); the retained representative is the **first-seen** path (deterministic), but identity — not the path string — is the dedup key (§2.3.2). Applied as the set is built in §1.1/§2.4 (P3.5).
- [ ] **P3.8** [RUST] Build `fs_guard::is_safe_output` — write-target link-safety against the frozen source set · §2.3.3 · G9 G48
  needs: P3.6
  > reject any candidate `final` whose resolved identity equals a frozen **source FILE**, or whose resolved parent resolves onto/into a source file's resolved path (§2.3.3 rule 2); the frozen set holds **files only**, so landing beside-source **inside a dropped folder is the normal correct case** and must NOT be rejected (§2.3.3 — guard is "resolves onto an original file?", not "under a dropped folder?"). On reject → divert (§2.7, P3.6), and the divert target is re-checked. Closes T7 (input-side symlink/junction); a G48 in-core fuzz target (P0.4.3) — incl. the §2.3.3 dangerous-path classes.
- [ ] **P3.9** [RUST] Build the parent-directory-handle safety primitive (TOCTOU-closed) · §2.3.3 · G29 G48
  needs: P3.8
  > open the parent dir handle FIRST (`O_DIRECTORY` Unix / `NtCreateFile`/`CreateFile2` dir handle Windows), verify the **open handle's** identity is not inside the frozen set (canonical-prefix containment on the handle's real path), then publish the leaf **relative to that same handle** — so the parent cannot be swapped to a symlink between check and write. The publish primitive (P3.3) accepts this dir fd/handle; `fs_guard::atomic_publish(parent_handle, tmp, leaf)` encapsulates the per-OS form (§2.3.3). G29 SAST (the `std::process`/path-validation rules N/A; the module is unsafe-free except the documented Windows handle FFI in the single allow-listed module, P0.4.2).
- [ ] **P3.10** [RUST] Build `fs_guard::output_name` — verbatim-stem + space-paren numbering candidates · §2.2.1 §2.10.1
  needs: P3.1
  > `output_name`: `stem` taken **byte-for-byte** (§2.10.1 — `OsString`, no `to_string_lossy` for any operation, no transliteration/ASCII-fold/emoji-strip), `ext` = target's canonical lowercase extension (`tsv`/`csv`); first candidate `stem.ext`, then `stem (n).ext` (n=1,2,3…) — the SSOT space-paren shape, never `_1`/`-1`/a hash; multi-dot stems preserved (`my.report.final` → `…final.tsv`). Candidates produced **lazily** for the §2.2.2 exclusive-publish loop (P3.15), never a directory-list max+1 pre-scan.
- [ ] **P3.11** [RUST] Build `fs_guard::check_path_limit` — per-OS component + total limits, fail-never-truncate · §2.2.3 §2.10.1 · G48
  needs: P3.10
  > validate the **resolved final** path length before the exclusive create: **Windows** total ≤ 260 (`MAX_PATH`, no long-path-aware assumption) + component ≤ 255 UTF-16; **macOS** component ≤ 255 UTF-8 bytes, total ≈ `PATH_MAX` 1024; **Linux** component ≤ 255 (`NAME_MAX`), total ≤ 4096 (`PATH_MAX`). When appending `(n)`/swapping the extension would overflow → emit `PathTooLong` (§2.8), **never truncate**. Windows: prepend `\\?\` manually for constructed numbered candidates (no "dunce inverse" — `dunce` only strips), but a user-facing path > 260 still fails clearly. Runs on the fully-resolved path **including any §2.7 divert** (P3.6). A G48 in-core fuzz target (NUL-path / `PATH_MAX`+1 bound-firing fixtures, P0.4.3).

---

### `crate::fs_guard`: the atomic, no-clobber publish OS primitives + FAT/exFAT divert

**Goal:** the real `fs_guard::atomic_publish` — the no-placeholder exclusive-rename per OS,
the durability fsync, the FAT/exFAT third-fallback that triggers a divert, and the §2.1.3
two-state crash invariant. This is the heart of what the walking skeleton proves.

- [ ] **P3.12** [RUST] Build the Unix single-call no-replace publish primitive (Linux/macOS, named) · §2.1.1 §2.1.2 · G48
  needs: P3.9
  > the dir-handle-relative create-only publish: **Linux** `renameat2(olddirfd, tmp, dirfd, leaf, RENAME_NOREPLACE)` (fails `EEXIST`); **macOS** `renameatx_np(olddirfd, tmp, dirfd, leaf, RENAME_EXCL)` (the macOS equivalent; macOS has NO `renameat2`/`RENAME_NOREPLACE` — the Linux spelling must not be used). Chosen at **runtime per destination** (an unsupported `EINVAL`/non-`VOL_CAP_INT_RENAME_EXCL` filesystem falls back, P3.13), never a build-time switch. No 0-byte placeholder at `final` ever (§2.1.2). G48 fuzz target on `is_safe_output`/`atomic_publish` (P0.4.3).
- [ ] **P3.13** [RUST] Build the Unix `link`+`unlink` fallback + its success-window residual handling · §2.1.2 §2.1.3
  needs: P3.12
  > the portable POSIX fallback `link(tmp, final)` then `unlink(tmp)` (fails `EEXIST`), used when the single-call no-replace primitive is unavailable/unsupported. Handle the **success-window residual** (after `link` succeeds, before `unlink` — both `final` and `*.part` briefly exist; the §2.1.3 link-form sub-state): `final` is complete+durable (Success); a failed `unlink` leaves a residual `*.part` reclaimed by the §2.6.4 sweep (annotated, not an item failure). On NFS treat an ambiguous rename result as name-may-be-taken → re-pick (§2.1.2).
- [ ] **P3.14** [RUST] Build the Windows dir-handle-relative create-only publish (`FileRenameInformationEx`) · §2.1.2 §2.3.3
  needs: P3.9
  > `NtSetInformationFile(tmpHandle, …, FileRenameInformationEx)` with `RootDirectory` = the verified parent-dir HANDLE, `FileName` = `leaf`, `Flags` **OMITTING** `FILE_RENAME_REPLACE_IF_EXISTS` → `STATUS_OBJECT_NAME_COLLISION` on collision (the Ex-class bitfield, not the boolean `ReplaceIfExists`). Closes the parent-swap race (plain path-string `MoveFileExW` re-resolves by path and does NOT). Atomicity comes SOLELY from the no-replace move; `MOVEFILE_REPLACE_EXISTING`/`ReplaceFileW` have **no caller** in v1. Bounded short-backoff AV-retry on transient `STATUS_ACCESS_DENIED`/`STATUS_SHARING_VIOLATION` → then `WriteFailed` (§2.8).
- [ ] **P3.15** [RUST] Build the numbering ↔ no-clobber retry loop (one bounded loop) · §2.1.2 §2.2.2
  needs: P3.12, P3.13, P3.14, P3.10
  > the single bounded loop where §2.2 numbering and the absolute no-clobber guarantee are the SAME thing: hand each lazy `output_name` candidate to the dir-handle-relative exclusive publish; on the exists-error bump the `(n)` suffix and yield the next candidate; cap ≈10 000 variants → too-many-collisions/`PathTooLong` failure (§2.8). The directory's real state at the instant of the kernel publish decides — not a stale scan (an optional cheap `symlink_metadata` pre-check may skip obviously-taken low numbers, but the **authority is always the kernel publish**, §2.2.2).
- [ ] **P3.16** [RUST] Build the durability sequence — `sync_all` + post-publish directory fsync · §2.1.1
  needs: P3.15
  > step 3 `tmp.sync_all()` (`fsync`/`FlushFileBuffers`) so bytes are durable **before** the rename; step 6 on Unix **fsync the containing directory** after the rename (and on the `link`+`unlink` path after `link`, the same dir-fsync — bytes already durable via the shared inode); Windows dir-fsync is a no-op (NTFS journaling), `MOVEFILE_WRITE_THROUGH` is best-effort metadata flush only. Atomicity does NOT depend on `WRITE_THROUGH` (§2.1.1).
- [ ] **P3.17** [RUST] Build the §2.14.3 EXDEV cross-volume fallback inside `fs_guard::atomic_publish` (copy-exactly-once → same-volume exclusive publish) · §2.14.3 §2.1.2 §2.8 · G48 G31
  needs: P3.16, P3.15, P3.20, P3.21
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P3.20`/`P3.21` point at the publish-temp naming model (`.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`, P3.20) + the lock-before-part run-lifecycle invariant (P3.21) defined later in document order — the EXDEV fallback places its cross-volume intermediate under the per-run scratch root using that `.part` naming so §2.6.3 can reclaim it, so DECISION C builds both first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
  > the §2.14.3 `[DECIDED]` reactive cross-device fallback `fs_guard::atomic_publish` tries the direct intra-volume no-placeholder publish (P3.14/P3.15) first and only on **EXDEV/cross-device** runs: (a) detect the cross-device failure; (b) place the engine output on a **named, swept other-volume kind-2 temp** (under the per-run scratch root, or carrying the `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` naming so §2.6.3 reclaims it — never an anonymous `$TMPDIR` `tempfile`), the lock-before-part ordering (P3.21) covering it; (c) **re-check `final`-volume free space against the intermediate (≈ output) BEFORE the copy** and fail `OutOfDisk` (§2.8) if it won't fit (this path's ~2× destination-volume peak the §1.10/§2.14.4 preflight does NOT model — mirror §2.7.2's late-divert "never assume it fits"); (d) copy the cross-volume temp into a **new same-volume temp** + `sync_all()` it; (e) publish that same-volume temp → `final` with the no-placeholder exclusive-rename (P3.12/P3.14), the §2.2 numbering retry **re-renaming the SAME already-copied intermediate** on a collision (the expensive cross-volume copy happens **EXACTLY ONCE**; only the cheap intra-volume exclusive-rename loops); (f) dir-fsync; the extra copy removed by §2.6. The cross-volume step is a **copy**, never a cross-volume `rename`; the only rename is intra-volume + exclusive. (The §2.14.2 `0o700` per-run kind-2 scratch-ROOT creation primitive this temp lands under is owned by P3.21's run-lifecycle ordering — the run-start `run-<RunId>/` create-then-lock step.)
- [ ] **P3.18** [RUST] Build the FAT/exFAT-class detection + `DivertReason::NoAtomicPublish` (Unix-only) · §2.1.2 §2.7.2 · G48
  needs: P3.13
  > detect a destination filesystem that supports **neither** the no-replace rename **nor** hardlinks (FAT/exFAT — `link()` → `EPERM`/`ENOTSUP`): via the OS filesystem-type/`statfs`-class query OR a one-shot capability probe (no-replace-rename → `EINVAL`/unsupported AND `link()` → `EPERM`/`ENOTSUP`). On Unix this is a per-location **DIVERT trigger** carrying `DivertReason::NoAtomicPublish` (§0.6) — the full §2.1 chain then runs on the hardlink-capable system disk (P3.6). **Windows is NOT diverted** — `MoveFileExW`-without-`REPLACE_EXISTING` is a true create-only move on FAT/exFAT too. A G48 bound-firing target.
- [ ] **P3.19** [TEST] Assert the two independent fault-injection invariants — the §2.1.3 all-OS crash/power-loss two-state invariant + the §2.1.2 Windows-only AV-retry · §2.1.3 §2.1.2 · G31 G15
  needs: P3.16, P3.14, P0.5.9
  > grouping shell over **two independent, separately-faileable** fault-injection scenarios that exercise different failure modes on different OS paths via different `#[cfg(test)]` seams and fail for independent reasons (a broken Windows AV retry must NOT block debugging the cross-OS crash invariant) — split per the same discipline as P3.46 (.1 FSM / .2 projection, "a bug in one is independent from a bug in the other; each fails a different check"). The parent is `[x]` only when both sub-boxes are (_format.md §2). This box (via its sub-boxes) is the activation target for BOTH P0.5.9 homes — the §2.1.3 atomicity-under-interruption home (→ P3.19.1) AND the §2.1.2 Windows-AV-retry fault-injection home (→ P3.19.2): this is the P3 box the P0.5.9 `→ activated in … P3` atomicity/AV-retry edge points at (`needs: P0.5.9`, the P0 home is `[x]` before the loop, mirroring the P2.127/P4.18.1 back-reference pattern). (`needs: P3.16` for the durability sequence the crash invariant is premised on + `P3.14` for the Windows publish path the AV-retry seam injects into.)
  - [ ] **P3.19.1** [TEST] Assert the §2.1.3 crash/power-loss two-state invariant — kill in the post-`sync_all`-pre-`rename` window (all 3 OS) · §2.1.3 · G31 G15
    needs: P3.16
    > a `#[cfg(test)]` fence in `fs_guard::atomic_publish` injecting a kill **specifically between `sync_all()` and the rename** (all 3 OS, premised on the P3.16 durability sequence) and asserting the on-disk state is **exactly one of** the §2.1.3 states (complete `final`, OR no `final` + a discardable `*.part`, OR — link-fallback only — `final` + a leftover `*.part`) — **never** a truncated/0-byte `final`. G31 source-unchanged leg corroborates no original was touched. The activation target for the P0.5.9 §2.1.3 atomicity-under-interruption home. (`needs: P3.16` for the durability sequence this invariant is premised on.)
  - [ ] **P3.19.2** [TEST] Assert the §2.1.2 Windows-only AV-retry fault-injection — transient `STATUS_SHARING_VIOLATION`/`STATUS_ACCESS_DENIED` → bounded retry → `WriteFailed` · §2.1.2 §2.8 · G31 G15
    needs: P3.14
    > a `#[cfg(test)]` seam in the Windows `FileRenameInformationEx` path (P3.14) injects a transient `STATUS_SHARING_VIOLATION`/`STATUS_ACCESS_DENIED` and asserts the **bounded short-backoff retry** fires and, when the violation persists past the cap, the publish surfaces `WriteFailed` (§2.8) — never a silent overwrite, never an unbounded spin. Windows-specific (`cfg(target_os="windows")`). The activation target for the P0.5.9 §2.1.2 Windows-AV-retry fault-injection home. (`needs: P3.14` for the Windows publish path the seam injects into.)

---

### `crate::run` & `crate::fs_guard`: temp ownership, cleanup & free-space restoration

**Goal:** run-owned `*.part` namespacing, the startup sweep that never deletes a live
instance's temp, and honest residue reporting — so a killed/failed/cancelled walking-skeleton
item leaves nothing (or surfaces the residue). Activation target for P0.5.9 temp-ownership.

- [ ] **P3.20** [RUST] Build the publish-temp naming + ownership model (`InstanceId`+`RunId`-encoded) · §2.6.1 §2.14.1 §3.5.6
  needs: P3.1
  > the kind-1 publish temp is a uniquely-named **sibling dotfile** in the destination dir — `…/<dest_dir>/.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` (`tempfile::NamedTempFile::new_in(final_dir)` / a `TempPath` rooted in `final_dir`), on `final`'s volume by construction (§2.14.1), **never** a system-temp file (§3.5.6 — the native engine's `out_tmp` is this dest-dir temp). `InstanceId`+`RunId` encode ownership so cleanup can tell its own temps from a concurrent instance's and resolve the exact owning lock (§2.6.1).
- [ ] **P3.21** [RUST] Build the lock-before-part run-lifecycle ordering invariant · §2.6.3 §2.14.1
  needs: P3.20
  > `crate::run` at run start: mint `RunId` → create `run-<RunId>/` under the central scratch root → acquire + OS-lock `.lock` → **only then** write the first `*.part`. This is the premise that makes "absent lock ⇒ dead ⇒ reclaimable" SAFE (so a concurrent sweeper never deletes a live foreign `*.part`, §2.6.3) — a §6 property-test target.
- [ ] **P3.22** [RUST] Build `crate::run::cleanup_item` / `cleanup_run` — own-prefix-scoped cleanup on every exit path · §2.6.2 · G31
  needs: P3.20
  > the per-exit-path cleanup table (§2.6.2): item-success (single-call → nothing to remove; link-fallback → `unlink(tmp)`); item-failure → remove that `tmp`; out-of-disk → remove partial + `OutOfDisk` (§2.8), batch continues; run-end → remove the recorded `final_dir` set's temps **by exact own prefix `.convertia-<thisInstanceId>-<thisRunId>-*.part`**, **never a bare `*.part` glob** (which would delete a concurrent foreign instance's live temp — SSOT). Recording the actual `final_dir` per written item (incl. divert/cross-volume) is what makes run-end enumerate every dir a temp landed in.
- [ ] **P3.23** [RUST] Build `crate::run::sweep_stale` — startup sweep with held-lock as the sole delete gate · §2.6.3
  needs: P3.21
  > on startup glob `convertia/scratch/<*>.<*>/run-*` across ALL instance dirs; liveness via a **NON-BLOCKING try-lock** (Unix `flock(LOCK_EX|LOCK_NB)`/`fcntl(F_SETLK)`; Windows `LockFileEx` with `LOCKFILE_FAIL_IMMEDIATELY|LOCKFILE_EXCLUSIVE_LOCK` — bare forms BLOCK, wrong here). Would-block ⇒ LIVE ⇒ untouched; immediate-acquire ⇒ DEAD ⇒ removed (then release). Held-lock is the **sole** delete predicate — never mtime/PID alone (PIDs are reused, §7.1.2). Close the create-then-not-yet-locked window: skip lockless run dirs within a short mtime grace window.
- [ ] **P3.24** [RUST] Build the opportunistic destination-resident `*.part` reclaim (cross-instance lock-addressable) · §2.6.3
  needs: P3.23
  > because kind-1 `*.part` live in destination dirs (not the central scratch root) and §7.4 persists no destination set, reclaim them at (a) run-end/same-session retry and (b) **opportunistically** before any later write into a dest dir: remove a sibling stale `.convertia-*.part` only when its owning run is **dead** — resolve the exact owning lock `convertia/scratch/<InstanceId>.*/run-<RunId>/.lock` from the embedded ids (held ⇒ keep; free/stale/**absent** ⇒ dead ⇒ reclaim). Recognise the **pre-RunId probe residue** `.convertia-<InstanceId>-probe-<rand>.part` (no RunId/jobId) → resolve liveness by `InstanceId` alone (§2.6.3 / §2.7.2).
- [ ] **P3.25** [RUST] Build cleanup-failure honesty — `CleanupResidue` surfacing, never a silent clean success · §2.6.4 §1.12 §2.8.2 · G31
  needs: P3.22, P3.68
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P3.68` points at the §2.8.2 message catalog at end-of-P3 (later document order) — the `CleanupResidue`/"With residue" strings this box surfaces live in that catalog, so DECISION C builds P3.68 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
  > if removing a temp fails: success-with-undeletable-`tmp` → success stands + a `residue` annotation; a failed item whose partial couldn't be cleaned → reported **Failed** WITH `CleanupResidue` naming the path (never a clean success); a Cancelled item whose publish temp survived the §1.7 bounded group-kill confirm-wait → carries a `CleanupResidue` + the §2.8.2 "With residue" tail (§2.6.4 case 3). `CleanupResidue { item, residue_path }` flows into `RunResult.cleanup_incomplete` (§1.12). The `{path}`-substituted `CleanupResidue` string + the "With residue" tail live in `crate::outcome` (the §2.8.2 catalog row authored in **P3.68** — `needs: P3.68`, followed in place by DECISION C).

---

### `crate::detection`: the §1.2 layered-detection framework bootstrap (CSV/TSV path)

**Goal:** the §1.2 detection framework exists (magic-sniff + container/text/encoding
classification scaffolding) and classifies the walking-skeleton types — enough to drive
drop→detect→group. P5–P7 later add only per-format signatures. Activation target for the
P0.5.7 KAT convention and the P0.4.3 detect fuzz target.

- [ ] **P3.26** [RUST] Build the layered-detection dispatcher skeleton (magic → container → text → structural-peek) · §1.2 §2.12.4 · G29
  needs: P3.1
  > the §1.2 strategy order as a dispatcher: (1) magic-byte/signature sniff on a bounded **first-4-KiB** header window; (2) container introspection seam (ZIP/OLE/`ftyp`/gzip — stubbed for P3, filled by P5–P7); (3) text classification; (4) bounded structural-peek for `notes`/`dims`. All steps are bounded reads in **memory-safe Rust** with no third-party C/C++ decoder, so detection runs **in-core** (§2.12.4 absolute satisfied) — no isolation subprocess for a sniff. Only the text-classification path needed for CSV/TSV is live in P3; the rest are typed seams.
- [ ] **P3.27** [RUST] Build the text/encoding classification (BOM → UTF-8 → codepage fallback) · §1.2 §2.10.2
  needs: P3.26
  > confirm bytes decode as text (BOM → strict UTF-8 → single-byte codepage fallback, e.g. `chardetng`); the encoding heuristic stays **in-core** (memory-safe, bounded, no C/C++ decoder, §2.12.4). Produces `CollectedSummary.encoding_hint` (e.g. "Windows-1252") for the §1.4 summary line. Text encoding is detected, **never assumed from the extension** (§2.10.2).
- [ ] **P3.28** [RUST] Build CSV-vs-TSV delimiter detection (content over name) · §1.2 §2.10.2
  needs: P3.27
  > delimiter sniff over the bounded sample: a consistent tab-delimited file is **TSV** even if named `.csv` (content over name, §1.2/spreadsheets.md); a consistent comma file is CSV; produce `CollectedSummary.delimiter_hint`. Ambiguous (no consistent delimiter) → `Uncertain` (never silently extension-fall-back). Grouping keys on the resulting **`UserFacingFormat`** (CSV ≠ TSV, delimiter-determined, §1.3).
- [ ] **P3.29** [RUST] Populate the `DetectionOutcome` result model + outcome rules · §1.2
  needs: P3.28
  > emit `DetectionOutcome::Recognized { format, confidence: Confidence, dims: None }` for CSV/TSV (non-raster → `dims: None`); `UnsupportedType { detected }` / `Uncertain { best_guess }` / `Empty` / `Unreadable { reason: ReadFailure }` for the others. Outcome rules: unsupported/uncertain/empty/unreadable are **never** offered a target list and never extension-fall-back (§1.2); a `.csv`-that-is-really-TSV converts as its **detected** type. `Confidence { High, Low }` — `Low` never silently falls back to the extension.
- [ ] **P3.30** [TEST] Stand up the §1.2 detection KAT first entries (CSV/TSV) · §1.2 §6.4.1 · G15
  needs: P3.29, P0.5.7
  > add the first `tests/detect-kat.toml` entries pinning canonical CSV/TSV (and a `.csv`-that-is-TSV, an ambiguous→`Uncertain`) files to their exact `FormatId`, read by the G15 unit test so §6.4.1's claim is machine-enforced at L2. This is the P3 box the P0.5.7 `→ activated in P3` KAT-convention edge points at (`needs: P0.5.7`, the P0 home is `[x]` before the loop, mirroring the P3.67 `needs: P0.5.8` activation-target pattern). A pure-Rust detection-fuzz target on `crate::detect` is the P0.4.3/G48 leg (registered there).

---

### `crate::fs_guard`: the §1.1/§2.4 freeze + §2.7 destination & per-location divert

**Goal:** the frozen source set (§2.4), the recursive folder walk (§1.1), and the §2.7
destination model incl. per-location writability/ephemeral/FAT-exFAT divert — so the walking
skeleton can ingest a CSV folder and land output beside-source or diverted, on all 3 OS.

- [ ] **P3.31** [RUST] Build the §1.1 recursive folder walk + hidden/system filter + per-item-failure-continues · §1.1 §2.4.1
  needs: P3.7
  > the Rust-side depth-first walk (`walkdir`; symlinked dirs not followed as a traversal step, loop-safety); ignore dotfiles + `.DS_Store`/`Thumbs.db`/`desktop.ini` + Windows hidden/system-attribute entries (fixed constant, §1.1); a per-item read/detect failure mid-walk yields a `SkippedItem` and the walk **CONTINUES** (only a `cancel_ingest`/fatal-walk-root error stops it). Retain dropped root(s) for §2.7 subtree re-creation + open-folder.
- [ ] **P3.32** [RUST] Build the §2.4 freeze point + zero-byte/unreadable-at-intake = Skipped · §2.4.1 §2.4.2 §1.1
  needs: P3.31
  > snapshot the set **eagerly and once** into an immutable `Vec<DroppedItem>` (conversion iterates the snapshot, never re-reads the dir — the structural no-self-feeding defence, §2.4.2); de-dup by `FileIdentity` (P3.7). A 0-byte/unreadable-at-intake item → `Skipped(SkipReason::Empty|Unreadable)` in the §1.4 summary (NOT silently dropped, NOT counted `failed`) — distinct from the **turn-time** unreadable/gone = `Failed` (§1.1). Closes T8 (no self-feeding) at the freeze.
- [ ] **P3.33** [RUST] Build `fs_guard::location_status` — writability + ephemeral classification (cached per-dir) · §2.7.2 · G31
  needs: P3.1
  > the **writable** test: `create_new` a throwaway probe file (`.convertia-<InstanceId>-probe-<rand>.part`, pre-RunId, §2.7.2) then remove it — confirms the dir accepts a create (NOT the §2.1 publish primitive; do not share the helper). Probe **lazily, cache per-directory** within the run. **Ephemeral** test: under `%TEMP%`/`GetTempPathW` (Win), `$TMPDIR`/`/tmp`/`/var/folders` (macOS), `$TMPDIR`/`/tmp`/`/var/tmp`/`/run/user/<uid>` (Linux) → divert. Probe-cleanup-failure → still **writable**, logged only, never a divert. The per-dir cache is a planning **hint**, not a commitment (P3.36 re-checks at write).
- [ ] **P3.34** [RUST] Build the §2.7 destination modes — beside-source + user-chosen-root subtree re-creation (create-only, ancestor-by-ancestor) · §2.7.1 §1.8
  needs: P3.9, P3.33
  > **beside-source (default):** output in the source's parent dir. **User-chosen root `D`:** re-create the dropped-root-relative subtree `D/sub/dir/file.<tgt>` (never flattened); each missing ancestor created **create-only** (`mkdir`, never `mkdir -p`-that-accepts-an-existing-file), ancestor-by-ancestor, then the deepest-created dir's handle is opened + link-safety-verified (P3.9) before the leaf publish (§2.7.1 / §2.3.3 ordering). The common root = deepest dir containing all frozen sources (computed at freeze).
- [ ] **P3.35** [RUST] Build the §2.7.3 divert target resolution + re-test of the divert root · §2.7.3 §2.7.4
  needs: P3.33
  > on an unwritable/ephemeral/`NoAtomicPublish` location, divert that source's output (per-location, not whole-batch) to **Downloads → Documents fallback** via `PathResolver` (`download_dir()`/`document_dir()`), overridable by the user-chosen root. The divert target is itself run through `location_status` (incl. the FAT/exFAT test, P3.18) — if it too is ephemeral/unwritable/FAT-exFAT → fail clearly `WriteFailed` (§2.8), never divert onto a purgeable/another-FAT volume. Diverted outputs de-collided by the same §2.2 numbering; summary maps each output to its source (§2.7.4).
- [ ] **P3.36** [RUST] Build the late-divert path — post-probe read-only flip re-runs the full safety chain · §2.7.2 §2.7.5 · G31
  needs: P3.35, P3.15, P3.11
  > when the real §2.1 publish fails for a **writability** reason (USB pulled / share dropped / permission flip after the cached probe), treat the location as unwritable and **late-divert** to the §2.7.3 target **before** reporting failure — re-running the full chain on the divert target: §2.3.3 `is_safe_output`, §2.2.3 path-limit re-checked against the divert **absolute path**, §2.14.4 free-space re-checked against the divert **volume**, then the §2.1 publish. A non-writability error (`OutOfDisk`) is NOT a divert trigger. The divert path is **not degraded** — every guarantee runs identically (§2.7.5, the SSOT Principle-5 assertion this proves end-to-end).

---

### `crate::fs_guard`/`run`: the §2.1.1 write-sequence assembly + §1.8 OutputPlan (CSV→TSV)

**Goal:** the §1.8 `OutputPlan` computation and the §2.1.1 7-step write sequence wired
together, plus the §2.5 re-run detection — the orchestration that consumes detect+plan and
produces an atomic publish for the walking-skeleton job.

- [ ] **P3.37** [RUST] Build the §1.8 `OutputPlan` computation (directory-based, no pre-baked `final_path`) · §1.8 §2.7
  needs: P3.34, P3.35
  > compute `OutputPlan { job, final_dir, diverted: Option<DivertReason>, base_name, extension, publish_temp_dir }` per job before any write, applying the §2.7 rules: resolve `final_dir` (beside-source or diverted), set `publish_temp_dir = final_dir` (the sibling-dotfile temp on the same volume, §2.14.1). **Directory-based by design** — the exact final name + `(n)` numbering is resolved at write time on the resolved real file (P3.15), **never** pre-baked into a `final_path` string (a pre-numbered path reintroduces the §2.1.2 TOCTOU race). No `crosses_volume` field (EXDEV detected reactively at publish, §2.14.3).
- [ ] **P3.38** [RUST] Assemble the §2.1.1 per-item write sequence (pick-temp → engine-writes → sync → resolve-late → publish → dir-fsync → cleanup-on-error) · §2.1.1 · G31 G32
  needs: P3.37, P3.16, P3.22
  > wire the 7 steps in order: (1) pick the publish-temp on `final`'s volume (P3.20); (2) the native CSV/TSV engine writes into `tmp` (P3.8); (3) `tmp.sync_all()`; (4) resolve `final` + the no-clobber decision **as late as possible**; (5) the no-placeholder exclusive-rename publish (P3.15); (6) durability dir-fsync (P3.16); (7) on any error in 3–6, remove `tmp` — `final` was never created (P3.22). Exit-verification: success **only if** the temp output exists and is non-empty (§1.7). G31/G32 source-unchanged + output-validity bind to this pair (activation target for P0.5.5).
- [ ] **P3.39** [RUST] Build the §2.5 re-run equivalence key + in-session ledger (the sole firing signal) · §2.5.1 §2.5.2
  needs: P3.37
  > `EquivKey = hash(source_identity, target_format, effective_settings_canon)` — **no destination component** (v1 verdict destination-independent, §2.5.1); `effective_settings_canon` is the fully-defaulted option set serialised order-independently. `crate::run` keeps an in-memory `HashSet<EquivKey>` (cleared on quit, nothing persisted, §7.4) — a second identical drop **same session** → the prompt fires. Disk presence is a **corroborator only, never fires alone** (an existing same-named file is an ordinary collision → silent numbering across sessions, §2.5.2). Accept the documented vanished-output / changed-destination edges.
- [ ] **P3.40** [RUST] Wire re-run detection into C4 + the §2.5.3 never-overwrite fallback · §2.5.2 §2.5.3 §1.8
  needs: P3.39
  > compute re-run equivalence during **C4 `plan_output`**, returned in `OutputPlanPreview.rerun` (so the UI enters RerunPrompt before Convert); C6 carries the user's `RerunDecision` (Skip default / FreshCopy → ordinary §2.2 numbering, **never** a replacing publish). When equivalence can't be determined (renamed/moved prior output, new session) → fall through to §2.2 silent next-free-variant numbering — the failure mode is a harmless extra numbered copy, **never** an overwrite (which §2.1's exclusive-create makes impossible regardless, §2.5.3).

---

### The native CSV→TSV engine (§3.5.6, in-core MIT Rust) + §1.7 InProcessNative lifecycle

**Goal:** the actual in-core transform — single streamed pass, CSV-injection-safe, RFC-4180
re-quoting — and its §1.7 `InProcessNative` lifecycle (self-reported progress, cooperative
cancel, wall-clock timeout). This is the one engine the walking skeleton runs.

- [ ] **P3.41** [RUST] Build the streamed CSV/TSV transform pass (encoding-normalise → delimiter-swap → RFC-4180 re-quote) · §3.5.6 §2.10.2
  needs: P3.28, P3.5
  > a single streamed pass: detect encoding/delimiter (P3.27/P3.28) → re-encode to **UTF-8 (no BOM default)** → swap delimiter → **RFC-4180 re-quote** where a field contains the new delimiter/quote/newline → write to `out_tmp`. Use a real RFC-4180 reader (the `csv` crate). MIT (own code, no §3.6 concern). Both directions (CSV→TSV and TSV→CSV); the offered non-diagonal default for a CSV source is **TSV** (same-format CSV diagonal excluded from tiles, §1.5).
- [ ] **P3.42** [RUST] Build the CSV-injection-safe literal-preservation rule · §3.5.6 · G32
  needs: P3.41
  > leading `= + - @` stay **literal text** (never re-interpreted as a formula) — the CSV-injection-safe guarantee (§3.5.6); the G32 output-validity reader asserts CSV-injection literal-preservation (P0.5.6) over the corpus, so this is the behaviour that gate binds to.
- [ ] **P3.43** [RUST] Build the §1.7 `InProcessNative` self-reported progress (`progress_tx` → `ItemProgress`) · §1.7 §1.11 §3.2.2
  needs: P3.41, P3.4
  > no stdout to line-read → §1.7 attaches **no** line-reader and instead passes a bounded `tokio::sync::mpsc::Sender<f32>` (`progress_tx`) into the `spawn_blocking` executor; the sync loop `blocking_send(bytes_processed / source_size)` at each N-KB chunk; §1.7 forwards every received fraction as one `ConversionEvent::ItemProgress { runId, itemId, fraction, stage }` — wire-indistinguishable from every other engine (§1.11). Sub-100-KB inputs → a single `1.0` start→done tick (indistinguishable from `CoarseSpawnDone`). Bounded channel = natural back-pressure.
- [ ] **P3.44** [RUST] Build the §1.7 cooperative cancel (poll token at chunk boundary, drop `out_tmp`) · §1.7 §2.1
  needs: P3.43
  > the sync loop polls the job's `CancellationToken` at every N-KB chunk boundary; on cancel it stops mid-stream, **drops the `out_tmp` `TempPath`** (deleted on drop, §3.2.2) → `Cancelled` with no partial leftover — the "cleanly discards the one in progress" guarantee reached **cooperatively** (no kill step to sequence; the §2.6 group-kill step is a no-op for this engine, §1.7 InProcessNative sub-case).
- [ ] **P3.45** [RUST] Build the §1.7 wall-clock timeout + wedged-uninterruptible-read bound · §1.7 §0.9 §2.12.4
  needs: P3.44
  > a §0.9-owned wall-clock timeout (tight for this light engine) wraps the sync call; on expiry the loop is cancelled cooperatively → `Failed(EngineHang)`, run **CONTINUES**. The wedged-uninterruptible-read caveat: the abandoned thread MUST NOT exhaust the `spawn_blocking` pool — the pool is **bounded with headroom** above the global degree, AND/OR reads go through a **bounded chunked reader with a short per-read deadline**, so a handful of wedged reads degrade gracefully (those items fail, the batch finishes). The §1.10 input-size guard bounds CSV-expansion DoS (in-core untrusted-byte but pure bounded Rust, §2.12.4).

---

### Orchestrator & IPC: the §1.9 lifecycle + the §0.4 command/event wiring (slice scope)

**Goal:** the orchestrator that builds the batch from the frozen set, drives `JobState`,
fans progress over the Channel, and the §0.4 commands the slice needs — so a real
drop→convert round-trip runs through the typed IPC surface.

- [ ] **P3.46** [RUST] Build the §1.9 job/batch lifecycle FSM + the Running→Failed wire-ErrorKind projection · §1.9 §2.8
  needs: P3.45, P3.38
  > the two independent concerns decomposed into sub-boxes (a bug in the FSM transitions is independent from a bug in the internal-kind→wire-kind projection; each fails a different check). The parent is `[x]` only when both sub-boxes are (_format.md §2). A worker-thread panic is caught at the §2.13 boundary as a clean per-item `Failed` (the panic-boundary body is P4; P3 wires the per-item isolation seam). The forward `needs: P3.68` edge (the §2.8.2 catalog this maps INTO) is carried **only** on the .2 sub-box that renders into the catalog — not duplicated on this parent (the structured Forward-ref note sits on .2).
  - [ ] **P3.46.1** [RUST] Build the §1.9 job/batch FSM (Pending→Running→{Succeeded|Failed|Cancelled|Skipped}) + deterministic queue order · §1.9
    needs: P3.45, P3.38
    > pure orchestration: drive `JobState` transitions (`Pending → Running → {Succeeded|Failed(kind)|Cancelled}`; `Skipped` set at construction, never enters the queue, terminal); deterministic collected/traversal queue order, no reordering (§1.9). No taxonomy/serialization here — the internal-kind→wire-kind projection is P3.46.2.
  - [ ] **P3.46.2** [RUST] Build the Running→Failed internal-`ConversionErrorKind` → wire-`ErrorKind` projection (rendered by the P3.68 catalog) · §2.8
    needs: P3.46.1, P3.68
    > **Forward-ref note (DECISION-C ordering inversion):** `needs: P3.68` points at the §2.8.2 message catalog at end-of-P3 (later document order) — this projection renders INTO that catalog's `OutcomeMsg.text`, so DECISION C builds P3.68 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
    > the taxonomy/serialization leg: `crate::run` maps `InvocationResult::Failed(kind)` (internal `ConversionErrorKind`) to the wire `ErrorKind` via `ErrorKind::from(kind)` (the `From` impl owned by `crate::outcome`) and renders the per-item `OutcomeMsg::Failure { text }` from the **§2.8.2 catalog (P3.68)** **before** the state is recorded / a row or event emitted (§1.9). Sits between the FSM (P3.46.1) and the P3.68 string catalog — a missing enum variant fails here, a wrong transition fails in P3.46.1. (`needs: P3.68` — the §2.8.2 string catalog this projection renders into `OutcomeMsg.text`, built at end-of-P3 + followed in place by DECISION C.)
- [ ] **P3.47** [RUST] Materialise pre-flight skips into the batch at C6 construction (non-queue `Skipped` records) · §1.9 §1.12
  needs: P3.46, P3.32
  > at C6 the orchestrator builds the `Batch` from the frozen `CollectedSet`, creating for **every `SkippedItem` in `CollectedSet::Single.skipped`** a `ConversionJob` with `JobState = Skipped(reason)` set at construction (reason copied from `SkippedItem.reason`) over the §1.1 single id space — these never enter `Pending`, receive **no** Channel events, and are terminal at construction. This is the single anchor preventing a skip from being lost between the `CollectedSet` and the §1.12 projection (§1.9).
- [ ] **P3.48** [RUST] Wire the C6 `start_conversion` run + the `ConversionEvent` Channel fan-out · §0.4.1 §0.4.2 §1.11
  needs: P3.46
  > C6 creates a `RunId`, enqueues the batch (§0.9), spawns the in-core worker, returns the `RunId` immediately, and streams `ConversionEvent`s: `RunStarted { totalItems = QUEUED-eligible count, willReencode: false }` (CSV/TSV is never re-encode), `ItemStarted`, `ItemProgress` (P3.43), `ItemFinished { outcome }`, `BatchProgress { done, total }` (denominator = queued items only, so a skip never holds the bar below 100%), terminal `RunFinished(RunResult)`. Pre-flight skips emit **no live** `ItemFinished{Skipped}` — terminal-projection only (§0.4.2).
- [ ] **P3.49** [RUST] Implement C1 `ingest_paths` / C3 `get_targets` / C4 `plan_output` for the slice · §0.4.1 §1.3 §1.4 §1.5
  needs: P3.32, P3.29, P3.40
  > **C1** funnels drop/picker/launch-arg paths into the single freeze (P3.32), returns `CollectedSet` (`Single`/`Mixed`/`Unsupported`/`Uncertain`/`Empty`) projected per the §1.3 `group()` rule — incl. the lone-Unsupported / lone-Uncertain specificity + the `EmptyReport→skipped` projection. **C3** resolves the CSV/TSV `TargetOffer` (the offered set, the one pre-highlighted default = TSV for a CSV source, lossy flag, availability) from the registry (§1.5). **C4** computes the `OutputPlan` preview + `rerun` (P3.40) + the §1.10 preflight verdict; eager on `3→4`, debounced re-call on change (§0.4.1).
- [ ] **P3.50** [RUST] Implement C8 `get_run_summary` + the §1.12 `RunResult` projection (incl. pre-flight skips + the batch-summary string) · §0.4.1 §1.12 §2.8.2
  needs: P3.47, P3.25, P3.68
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P3.68` points at the §2.8.2 message catalog at end-of-P3 (later document order) — the §1.12 batch-summary strings this projection assembles live in that catalog, so DECISION C builds P3.68 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
  > the §1.12 run-end projection: `RunResult { collected_set_id, run_id, items, totals, cleanup_incomplete, common_root, divert_root }`; map each output back to its source; a fully-failed batch is a clear failure (derived `failed == total && total > 0`). Project **pre-flight skips** into `RunResult.items` as `ItemResult { state: Skipped(reason), output: None, reason: OutcomeMsg::Skipped{reason} }` (trivial copy, no lossy reverse map), counted in `Totals.skipped` (never `failed`); assemble the §1.12 **batch-level summary string** (All succeeded / Partial / All failed / Cancelled + the With-residue tail) from the §2.8.2 catalog (P3.68). C8 is the idempotent re-fetch (mirrors the terminal `RunFinished`). (`needs: P3.68` for the batch-summary strings, followed in place by DECISION C.)
- [ ] **P3.51** [RUST] Implement C9 `open_path` with the §7.7.3 RunResult-membership validation · §0.4.1 §2.7.4
  needs: P3.50
  > the DoD one-click open-folder/open-file action: the Rust handler **validates `path` against the current `RunResult`'s recorded outputs (or their common/divert root)** (§7.7.3 — the real gate; no `opener:*` WebView grant, §0.10) then calls `OpenerExt` (reveal/open) internally. `OpenKind::{Folder|File|RevealInFolder}`; "open folder" opens `common_root`, and when `divert_root` is `Some(..)` a second affordance opens the divert root (§2.7.4 / §1.12).
- [ ] **P3.52** [RUST] Wire C7 `cancel_run` / C13 `cancel_ingest` to the cooperative cancel + ingest-token · §0.4.1 §1.1 · G54
  needs: P3.48, P3.31
  > **C7** trips the run's §0.4.4 cancellation token → the in-core cooperative cancel (P3.44); already-finished items kept, in-progress discarded cleanly. **C13** trips the ingest-scoped `CancellationToken` keyed by the frontend-generated `CollectingId` (registered at C1 handler entry, dropped on **every** exit branch — no token leak) → the §1.1 walk stops cooperatively, discarding the partial un-frozen set (no cleanup obligation, no temp written during ingest). G54 governs the gate-plane integrity these handlers run under (no security claim of its own).

---

### Minimal walking-skeleton result UI (§5 minimal)

**Goal:** the minimal React/TS screen path that drives and renders the slice —
Idle/DropZone → Collecting → Confirm → Targets (TSV) → Destination → Converting → Summary —
exercising the generated `bindings.ts` IPC door. Full polish + the rich components are P4/P8.

> **P3↔P4 UI-seam model (DECIDED — the recommended model, stated in both phases):**
> P3 builds **intentionally-minimal, slice-only renderers** of these components (just
> enough to drive the CSV→TSV vertical slice end-to-end and prove the IPC door). P4's
> §P4.14 generic UX-correctness primitives (P4.63–P4.69 + P4.73/P4.74) **SUPERSEDE**
> them — P4 rebuilds each into the generic, option-declaration-driven, fully-a11y
> component P5–P7 register against. P4 does **NOT** extend the P3 renderers in place;
> it replaces them (the P3 versions are deliberately throwaway slice scaffolding, not a
> foundation P4 builds on). Each P4 UI box names the P3 box it supersedes + carries the
> `needs: P3.5x` edge so the loop builds the P3 slice-renderer first (it is the live UI
> until P4) and the supersede is explicit, never a silent double-build. This keeps the
> walking-skeleton philosophy literally true (P3 proves the slice; P4 generalises) and
> is the single owner of the answer to "does P4 rebuild or extend P3's UI?" — it
> **rebuilds (supersedes)**.

- [ ] **P3.53** [UI] Build the §5.2 walking-skeleton state machine (the slice subset) · §5.2 §5.8 · G57
  needs: P3.48
  > a finite-state reducer over the slice states: `Idle (1) → Collecting (2) → Confirm (3) → Targets+Destination (4/5) → [RerunPrompt (6)] → Converting (7) → Summary (8)`, plus the pre-flight `MixedDropRefusal (9)` / `Unsupported (10)` branches and the global `app://fault → AppFault (12)` wildcard edge. Driven by inbound IPC results/events (§5.8); the backend is the source of truth for facts. All user-facing literals via `strings/ui.ts` (English-only, G57, P0.4.6).
- [ ] **P3.54** [UI] Build the DropZone + C2a/C2b intake wiring (drop, click-to-browse, choose-folder) · §5.3 §5.4 §0.4.1
  needs: P3.53
  > **DropZone** in `Idle` (1) + the state-9 re-drop: native file-drop via the window-global `onDragDropEvent` (`paths: string[]`, NOT HTML5 DnD); click/Enter/Space → **C2a `pick_for_intake { kind: 'files' }`**; the "or choose a folder" affordance → **C2a `{ kind: 'folder' }`** (no `dialog:allow-open` grant — Rust-side `DialogExt`, §5.4/§0.10). No raw FS path transits the WebView for intake. A cancelled picker → `CollectedSet::Empty` → stays `Idle`.
- [ ] **P3.55** [UI] Build the Confirm gate (BatchSummary + FileList skip rows) · §5.3 §1.4
  needs: P3.54
  > **BatchSummary** renders the mandatory pre-convert gate (state 3): detected format + count ("N CSV files"); when `skipped` is non-empty, the passive one-line tally *"M file(s) weren't recognized and will be skipped"* (never blocks confirm, never silent). **FileList** (behind a "Show N files" disclosure) is the single owner of the per-item detail — eligible rows plain, skipped rows visually marked with their §2.8 reason; virtualised. `sampleNames`/`raw_path` are display-only, never re-submitted as intake (§5.3).
- [ ] **P3.56** [UI] Build the FormatPicker (TSV target) + DestinationBar (will-save-to + Change + preflight) · §5.3 §1.5 §2.7
  needs: P3.55
  > **FormatPicker** shows the offered target tile(s) with the one pre-highlighted **default = TSV** for a CSV source (descriptors from C3, no platform matrix hardcoded). **DestinationBar** always visible before Convert (state 5): the "**will save to …**" line (beside-source default / divert noted, from the C4 plan), the **Change destination** button (C2b `pick_destination` → C5 `set_destination`), and the **Convert** button **disabled** with a passive `Note` when `preflight.up_front_fail` is `Some(kind)` (the §2.8 string — fails fast up front).
- [ ] **P3.57** [UI] Build the RerunPrompt interstitial (Skip default / Fresh copy / Cancel) · §5.3 §2.5
  needs: P3.56
  > the one batch-level prompt (state 6), entered **only** from the C4 `rerun` flag (destination-independent, never re-entered on a C5 change): *"Already converted with these settings."* — **Skip (default, focused)** / **Make a fresh copy** / **Cancel** (Esc → back to Destination with the held plan intact). The choice becomes the `RerunDecision` carried into C6. Rendered as a focus-trapped `role="alertdialog"` over the inert-but-mounted Targets/Destination.
- [ ] **P3.58** [UI] Build the Converting screen (ProgressList real per-item + aggregate bar + Cancel) · §5.3 §1.11 §0.4.2
  needs: P3.57
  > **ProgressList** keyed by `itemId` over the `ItemProgress` payloads: real determinate per-item progress (never a bare spinner — the native engine self-reports a real fraction or an honest start→done, §1.11) + the aggregate `BatchProgress` bar; rows transition to terminal `Succeeded`/`Failed`/`Cancelled`/`Skipped`. **Cancel** button → C7 `cancel_run` → the `Converting (Cancelling…)` (7a) sub-state (button disabled, label "Cancelling…", a second Esc ignored) → `Summary` (partial) on backend confirm.
- [ ] **P3.59** [UI] Build the Summary screen (ResultSummary + OpenActions, split-divert two-button) · §5.3 §1.12 §2.6.4
  needs: P3.58
  > **ResultSummary** renders `RunResult`: per-item success/fail/skip with §2.8 reason strings, output→source map, fully-failed banner (never a quiet "done"); a residue item (`IpcError.residue != None` / in `cleanup_incomplete`) rendered **Failed** with the residue path + an optional "reveal residue" link (C9 `RevealInFolder`). **OpenActions** (Summary-only, not mid-run): "Open folder" → C9 `RevealInFolder` on `common_root`; on `divert_root = Some(..)` render TWO buttons ("Open source folder" + "Open saved-to folder") with the connector line — labels are real `strings/ui.ts` entries (§5.3). "Convert more" → `Idle`.
- [ ] **P3.60** [UI] Build the pre-flight refusal + fault screens (MixedDropRefusal, Unsupported, AppFault) · §5.2 §5.3 §2.13
  needs: P3.54
  > **MixedDropRefusal** (state 9): hard refusal listing formats+counts, an **active DropZone** as the primary re-drop action (→ `Collecting`) + Dismiss → `Idle`; no subset-convert. **UnsupportedNotice** (state 10): four variants (`Unsupported`/`Uncertain` + its note/`Unreadable`/`Empty` + skip tally), `aria-live="assertive"` heading, focus on Dismiss. **AppFaultNotice** (state 12): plain no-stack-trace "Something went wrong" + Start over (Ctrl/⌘+N), never fabricates per-item outcomes (§2.13). The CommandError inline slot handles a pre-run C3/C4/C5 reject without a full-screen takeover.

---

### Walking-skeleton end-to-end proof on all 3 OS

**Goal:** the slice is proven green end-to-end on Windows, macOS and Linux — the per-pair
integration test, the output-validity readers binding, and the cross-OS run that de-risks the
whole Tauri + atomic-publish + IPC stack early (the README walking-skeleton purpose).

- [ ] **P3.61** [TEST] Author the CSV→TSV / TSV→CSV corpus fixtures + the bound-firing CSV-expansion fixture · §6.4.5 §6.4.2 · G24a G16
  needs: P3.42, P0.5.11
  > committed corpus fixtures for both pairs (incl. a Windows-1252-encoded CSV, a quoted-field/embedded-delimiter CSV, a CSV-injection `=…`/`@…` leading-token file, a CJK/RTL-content file) under the §6.4.5 conventions; a deterministic **bound-firing** fixture exercising the §1.10 CSV-expansion input-size guard. Each fixture's SHA-256 joins the §6.4.2 corpus manifest **via the `stage-corpus` generator (P0.5.11) in the same commit** (G24a integrity, P0.5.4 — never a hand-computed hash); single-source helper, no inline duplication (§6.4.2). (`needs: P0.5.11` for the manifest generator the corpus landing invokes.)
- [ ] **P3.62** [TEST] Bind the G31/G32 output-validity + source-unchanged readers to the CSV/TSV pairs · §6.4.3 §2.5 · G31 G32
  needs: P3.61, P3.38, P0.5.5, P0.5.6
  > activate the P0.5.5/P0.5.6 invariants on these pairs (this is the FIRST binding of P0.5.5 source-unchanged + P0.5.6 output-validity to real CSV/TSV data — `needs: P0.5.5, P0.5.6`, the P0 homes are `[x]` before the loop, mirroring the P4.58 `needs: P0.5.6` activation pattern): **(a) SOURCE-UNCHANGED** — `sha256` of every corpus source unchanged before/after (the no-harm proof, G31); **(b) OUTPUT-VALIDITY** — the produced output passes a **real RFC-4180 reader** (the `csv` crate) + CSV-injection literal-preservation (NOT bare field-count) + non-empty/output≠input/size-plausibility (G32). Register CSV↔TSV in `tests/corpus/manifest.toml` with its byte-stable/lossy disposition + the determinism sub-assertion (`sha256(out1)==sha256(out2)`).
- [ ] **P3.63** [TEST] Author the per-pair integration runner end-to-end pass (drop→…→publish→summary) — activates the G23 handler→test gate for the first `convert_*` · §6.4.3 §6.5 · G15 G31 G23
  needs: P3.62, P3.50, P0.4.11
  > the §6.4.3 per-pair integration test driving the **real** vertical slice — frozen-set freeze → detect → C3/C4 → C6 convert → §2.1 atomic publish → `RunResult` summary — against the real temp FS (never mock the no-harm/`fs_guard` layer, the thing under test, P0.5.1); assert the published output exists at the expected beside-source/diverted name, the no-clobber numbering on a pre-existing collision, and the summary maps output→source. G15 unit+integration; the §6.5 ledger marks the pair `reliable` once it passes (the walking-skeleton's first ledger entry). **This is the partner test for the first `convert_*` IPC handler (the C6 `start_conversion` run path wired in P3.48 + the slice CSV/TSV transform P3.41) and so ACTIVATES the G23 handler→test bijection first fail-close from P3** (the contract authored in P0.4.11): once this test + that handler land, G23's `git ls-files` walk fail-closes for the CSV→TSV pair (no matrix row or bijection script needed — distinct from the G22 schema-parity leg, which first fail-closes in P4.59).
- [ ] **P3.64** [TEST] Add the §2.3/§2.4 link-safety + frozen-set + no-self-feeding integration cases · §2.3.2 §2.3.3 §2.4.2 §2.4.3 · G31 G48
  needs: P3.63, P0.5.9
  > integration cases proving the kernel on real links: a source reached via a symlink + its target both dropped → de-duped to one (§2.3.2); an output dir that is a symlink resolving onto a source → diverted, never clobbered (§2.3.3, T7); an output landing in a watched source folder does NOT expand/restart the batch (§2.4.2 snapshot, T8) and a concurrent/launch-time hand-off cannot inject into the frozen set (§2.4.3, T8); a hardlink/junction-to-source identity caught by dev+inode/file-index. These are the activation targets for the P0.5.9 T7/T8 homes (§2.4.2/§2.4.3) + the P0.4.3 `is_safe_output` fuzz — this is the P3 box the P0.5.9 `→ activated in … P3` T7/T8 link-safety/self-feeding edge points at (`needs: P0.5.9`, the P0 home is `[x]` before the loop, mirroring the P2.127/P4.18.1 back-reference pattern).
- [ ] **P3.65** [TEST] Add the FAT/exFAT-divert + atomicity-under-interruption cross-volume integration case · §2.1.2 §2.7.2 §2.14.3 · G31
  needs: P3.64, P3.18, P3.17
  > an integration case on a FAT/exFAT-class destination (Unix): the publish diverts with `DivertReason::NoAtomicPublish` to the hardlink-capable system disk and the full §2.1 chain holds there (Windows-FAT is NOT diverted and keeps the guarantee by construction, §2.1.2); a cross-volume (`EXDEV`) publish detected reactively exercises the §2.14.3 copy-exactly-once fallback **built in P3.17** (asserting the intermediate is named/swept, the destination-volume free-space re-check fires, and a numbering collision re-renames the already-copied intermediate, never re-copies); plus the §2.1.3 kill-in-the-rename-window assertion (P3.19) wired into the cross-OS run so the two-state invariant is proven on a real second volume.
- [ ] **P3.66** [TEST] Prove the full slice green on Windows + macOS + Linux (the cross-OS de-risk run) · §6.1.3 §6.7.1 · G15 G30
  needs: P3.65, P3.59
  > run the P3.63 integration runner + a thin smoke of the C1→C6→C8 round-trip on **all 3 OS** in the Lane-A matrix (G30 build-matrix, P0.4.10) — the README walking-skeleton purpose: de-risk the Tauri + atomic-publish + IPC stack early on every platform before any heavy engine. Assert the per-OS publish primitive (Linux `renameat2`/macOS `renameatx_np`/Windows `FileRenameInformationEx`) is exercised and green, and the slice builds + bundles on each OS (no engine/sidecar dependency to stage, §3.5.6).
- [ ] **P3.67** [TEST] Stand up `tests/fuzz_replay.rs` — replay every `fuzz/crashes/`+`fuzz/corpus/` file through the now-real detect + fs_guard (+ CSV/TSV) fuzz-target functions on the STABLE toolchain · §6.4.2 · G48 G24
  needs: P0.5.8, P3.30, P3.8, P3.41
  > the activation target for the P0.5.8 fuzz-crash replay convention (`→ activated in P3`): now that the real fuzz-target function BODIES exist — `crate::detect` (P3.30 KAT/sniff), `crate::fs_guard::resolve_identity`/`is_safe_output` (P3.6/P3.8), and the in-core CSV/TSV transform (P3.41) — wire `tests/fuzz_replay.rs` as a plain `cargo test` integration test feeding every committed `fuzz/crashes/`+`fuzz/corpus/` file directly to those target functions with **NO libFuzzer harness**, so it compiles + runs on EVERY platform incl. Windows under the **STABLE** toolchain (only the instrumented `cargo-fuzz` leg needs nightly — Linux/macOS, the P0.4.3/G48 leg). The G24 planted-positive (a committed crash fixture MUST fail the replay if its fix is reverted) binds here. This is the P3 box the P0.5.8 `→ activated in P3` edge points at (`needs: P0.5.8`, the P0-authored convention, satisfied since P0 is `[x]` before the loop).

---

### `crate::outcome`: the §2.8.2 message catalog + §2.9.1 lossy-note catalog (the single string home)

**Goal:** the actual canonical-English STRING TABLES `crate::outcome` is the "single
source of every conversion-outcome string" for (§2.0 line 43). P2 authored the
`ConversionErrorKind` ENUM (P2.18/P2.18.1), the `OutcomeMsg` TYPE + projection helper
(P2.20), and the `LossyKind` ENUM (P2.8.2); these two boxes author the kind→string
TABLES themselves — without them `OutcomeMsg::{Failure,Lossy}.text` has nothing to
produce. They sit at end-of-P3 (the outcome cluster's string-table leg) rather than
re-numbering the whole phase; the consumers (P3.25/P3.46/P3.50 + P4.64/P4.68/P8.19/
P8.20 + every per-pair fail-clearly / lossy-iff-flagged test) carry the forward
`needs:` resolved in place by DECISION C.

- [ ] **P3.68** [RUST] Author the §2.8.2 `ConversionErrorKind` → canonical-English message catalog (all item kinds + the 5 batch-summary strings + `{detected}`/`{platform}`/`{path}` substitution) · §2.8.2 §2.8 · G23 G57
  needs: P3.1, P2.18, P2.20
  > the §2.8.2 single-home string table in `crate::outcome` (the home §2.0 names): one canonical-English row per `ConversionErrorKind` item/app kind (`Corrupt`/`Empty`/`Unrecognized`/`UnsupportedType`/`UnsupportedPair`/`Unreadable`/`Gone`/`PasswordProtected`/`NoAudioTrack`/`TooBig`/`OutOfDisk`/`WriteFailed`/`PathTooLong`/`TooManyCollisions`/`EngineCrash`/`EngineHang`/`EngineError`/`PlatformUnavailable`/`QuarantinedByOs`/`CleanupResidue`/`InternalError`) with the `{detected}`/`{platform}`/`{path}` runtime-substitution wiring applied, PLUS the 5 **batch-level summary strings** (All succeeded / Partial / All failed / Cancelled / With-residue tail, assembled by §1.12). Produces `OutcomeMsg::Failure { kind, text }` (and the per-item `Skipped`/run-level chrome rows that ride the catalog) — the table P3.46 maps INTO via `ErrorKind::from`, P3.25 reads for the `cleanup_residue`/`CleanupResidue` row, P3.50 reads for the §1.12 projection, and P4.68/P8.19 render verbatim (UI never re-authors the string). Tone: plain/calm/never-blaming (SSOT *Fail clearly*); English-only (G57). A `#[test]` asserts every `ConversionErrorKind` variant has a non-empty catalog row (no unhomed kind). (`needs: P2.18` for the enum + `P2.20` for `OutcomeMsg`; the consumers carry the forward edge to this box per DECISION C.)
- [ ] **P3.69** [RUST] Author the §2.9.1 `LossyKind` → canonical-English note catalog (all variants + the `{w}`×`{h}` substitution) · §2.9.1 §2.9 · G23 G57
  needs: P3.1, P2.8.2, P2.20
  > the §2.9.1 single-home lossy-note table in `crate::outcome` (§2.9 "single home of every lossy-note string"): one calm canonical-English note per `LossyKind` variant (`image_lossy_codec`/`image_palette`/`image_downscale`/`image_alpha_flatten`/`image_animation_flatten`/`image_svg_raster`/`doc_pdf_reflow`/`doc_pdf_to_text`/`doc_html_render`/`doc_to_text`/`doc_simplified`/`sheet_to_delimited`/`xls_legacy_limits`/`text_encoding_narrowed`/`slides_to_pdf_flatten`/`office_roundtrip_approx`/`pptx_to_ppt_legacy`/`audio_lossy_target`/`audio_transcode`/`audio_lossy_origin`/`audio_bitdepth`/`audio_tags_dropped`/`video_reencode`/`video_alpha_lost`/`video_subs_dropped`/`video_to_gif`/`audio_downmix`), incl. the **`image_svg_raster` `{w}`×`{h}` substitution**; produces `OutcomeMsg::Lossy { kind, text }`. The table P4.64 surfaces verbatim in FormatPicker, P8.20 polishes the presentation of, and every P5/P6 lossy-iff-flagged test (P5.49/P5.50/P5.52/P5.73, P6.29/P6.74, …) asserts FIRES against. English-only (G57). A `#[test]` asserts every `LossyKind` variant has a non-empty note row. (`needs: P2.8.2` for the `LossyKind` enum + `P2.20` for `OutcomeMsg`; the consumers carry the forward edge per DECISION C.)

---

### Cross-phase reconciliation (the deferred P3→P2 contract `needs:`)

- [ ] **P3.70** [GATE] Wire the deferred P3→P2 contract `needs:` edges — domain types, detection-outcome, error model, IPC commands/events, state machine · §0.6 §0.4 · G7 G20
  needs: P2.4, P2.8, P2.13, P2.15, P2.18, P2.20, P2.22, P2.25, P2.26, P2.29, P2.37, P2.39, P2.120
  > the P3 instance of the cross-phase reconciliation obligation (the master plan-lint forbidden-string check is P4.76): P3 IMPLEMENTS the bodies behind P2's contracts, so every P3 box that consumes a P2-declared type/handler must carry the edge — P3.5 plan() reads the `Target`/`EngineId`/`EngineDescriptor` types (**P2.8/P2.13**); P3.29 populates `DetectionOutcome` (**P2.15**); P3.32 freezes into `Vec<DroppedItem>` (**P2.4**); P3.46 maps internal kind → wire `ErrorKind` via the `From`/`OutcomeMsg` projection (**P2.18/P2.20**); P3.68 authors the §2.8.2 catalog over `ConversionErrorKind`/`OutcomeMsg` (**P2.18/P2.20**) and P3.69 the §2.9.1 catalog over `LossyKind`/`OutcomeMsg` (**P2.8.2/P2.20**); P3.48 C6 fan-out over the `ConversionEvent` Channel (**P2.29/P2.37**); P3.49 C1/C3/C4 contracts (**P2.22/P2.25/P2.26**); P3.53 the §5.2 state machine over the three `app://` listeners (**P2.39/P2.120**). `needs:` these P2 boxes here so the §6 selection builds the P2 contract first (P2 is `[x]` before the loop reaches P3, so trivially satisfied — but the edge must RESOLVE, not dangle); no P3 box `>`-note defers a `needs:` with the P4.76-forbidden phrasing.

---

### `crate::run`/`crate::fs_guard`: the §2.14.1 temp-ownership security-test home

**Goal:** the [TEST] box that EXERCISES the §2.14.1 temp-ownership security invariants the
P3.20/P3.21 MECHANISM builders produce — the Unix mode bits, the Windows per-run scratch-root
DACL, cleanup-on-fault/kill leaving no orphan, and the Windows AV-lock-during-cleanup path. It
is the activation target for P0.5.9's §2.14.1 temp-ownership home (P3.20/P3.21 remain the
MECHANISM home). Placed at end-of-phase (after the P3.68/P3.69 string-table leg + the P3.70
reconciliation box, the file's established end-of-P3 placement convention) so it stays a
single gap-free id (`.71`) rather than re-numbering the whole P3 sequence + every cross-phase
reciprocal reference to P3.70.

- [ ] **P3.71** [TEST] Assert the §2.14.1 temp-ownership security invariants (grouping shell — four independent, separately-faileable OS-specific assertions) · §2.14.1 §2.6.3 §2.6.4 · G31 G15
  needs: P3.20, P3.21, P3.22, P3.23, P3.24, P0.5.9
  > grouping shell over the security-TEST home that EXERCISES the §2.14.1 temp-ownership invariants the P3.20 publish-temp-ownership + P3.21 lock-before-part MECHANISM builders produce (those stay the mechanism home). The four assertions target **different OS + syscall surfaces and fail independently** (a Unix mode-bit bug is unrelated to a broken Windows DACL or an orphaned-scratch failure), so each is its own sub-box per the same discipline as P3.46/P3.19/P1.62; the parent is `[x]` only when all four sub-boxes are (_format.md §2). The activation target for the P0.5.9 §2.14.1 temp-ownership security-TEST home — this is the P3 box the P0.5.9 `→ activated in … P3` §2.14.1 temp-ownership security-TEST edge points at (`needs: P0.5.9`, the P0 home is `[x]` before the loop, mirroring the P2.127/P4.18.1 back-reference pattern). G31 source-unchanged corroborates no original is touched. (`needs: P3.20`+`P3.21` for the ownership/lock mechanism, `P3.22`+`P3.23`+`P3.24` for the cleanup/sweep/reclaim paths the sub-boxes assert.)
  - [ ] **P3.71.1** [TEST] Assert the Unix `0o700`/`0o600` mode bits on per-run scratch + publish temps · §2.14.1 · G31 G15
    needs: P3.20, P3.21
    > **(a) Unix mode bits** — the per-run kind-2 scratch ROOT is created `0o700` and every kind-1 `*.part` publish temp is `0o600` (assert the on-disk `st_mode` permission bits, all Unix); independent of the Windows DACL/AV-lock paths.
  - [ ] **P3.71.2** [TEST] Assert the Windows per-run scratch-root DACL is locked to the current-user SID · §2.14.1 · G31 G15
    needs: P3.21
    > **(b) the Windows per-run scratch-root DACL** — the run-start `run-<RunId>/` scratch root grants access **only to the current-user SID** via an explicit restrictive DACL at create (assert via `icacls`/the security-descriptor query that no broader principal is granted). Windows-specific.
  - [ ] **P3.71.3** [TEST] Assert cleanup-on-fault/kill leaves no orphaned scratch dir (all OS) · §2.6.3 §2.6.4 · G31 G15
    needs: P3.22, P3.23, P3.24
    > **(c) cleanup-on-fault/kill leaves no orphaned temp** — inject a fault/kill on a representative exit path and assert `cleanup_item`/`cleanup_run` (P3.22) + the startup sweep (P3.23) + the opportunistic reclaim (P3.24) reclaim every own-prefix temp, leaving zero orphan (own-prefix-scoped, never a bare `*.part` glob that would touch a foreign live temp). All OS.
  - [ ] **P3.71.4** [TEST] Assert the Windows AV-lock-during-cleanup retry / `MoveFileEx` deferred-delete path · §2.6.4 · G31 G15
    needs: P3.22
    > **(d) the Windows AV-lock-during-cleanup path** — a held temp during cleanup yields `ERROR_SHARING_VIOLATION` → assert the bounded retry-after-release OR the `MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)` deferred-delete fallback fires (never a silent leak, never an unbounded spin). Windows-specific.

---

### `cargo-mutants` first informational pass over the no-harm/atomicity/no-misroute kernel

**Goal:** the [GATE] box that RUNS the P0.5.10-authored scoped `cargo-mutants` gate now that
the kernel crate BODIES exist — the first informational pass over `crate::fs_guard` +
`crate::detect` + `crate::outcome`, initialising the per-crate ratchet + the `gate-status.md`
informational entry. It is the activation target for P0.5.10's `→ activated in P3+` note
(`needs: P0.5.10`), mirroring the P3.67 fuzz-replay activation box: P0 authored the
CONTRACT + ratchet shape; this box is the runnable first pass against the real bodies that
the kernel-body boxes build, so the gate is enforced rather than authored-then-never-run.
Placed at end-of-phase (after the P3.68/P3.69 string-table leg + P3.70 reconciliation +
P3.71 temp-ownership test home, the file's established end-of-P3 convention) so it stays a
single gap-free id (`.72`) rather than re-numbering the whole P3 sequence.

- [ ] **P3.72** [GATE] Run the first informational `cargo-mutants` pass over `crate::fs_guard` + `crate::detect` + `crate::outcome`, initialise `max_survived_mutants.toml` per crate + the `gate-status.md` informational entry · §6.4 · G15 G24
  needs: P0.5.10, P3.6, P3.8, P3.18, P3.29, P3.46, P3.68, P3.69
  > the activation target for the P0.5.10 scoped mutation-testing gate (`→ activated in P3+`, "needs the kernel crates"): now that the no-harm/atomicity/no-misroute kernel BODIES exist — `crate::fs_guard` (`resolve_identity`/`is_safe_output`/`atomic_publish`/the FAT-exFAT divert, P3.6/P3.8/P3.18 + P3.15–P3.16), `crate::detect` (P3.29), and `crate::outcome` (the §2.8.2/§2.9.1 string-table leg P3.68/P3.69 + the §2.8 projection P3.46) — run the FIRST **informational** `cargo-mutants` pass scoped to those three crates, emitting a per-crate survived-mutant report and **initialising the decrease-only `max_survived_mutants.toml` ratchet per crate at the first-run count** (P0.5.10's activation criteria), plus writing the dated `informational` entry in `docs/process/gate-status.md` (plan-lint check 23). Line coverage proves a line ran, not that a test would CATCH a regression there — this is the kernel gate that proves the no-harm tests bite. Owner-decidable informational→required like G17b (the owner flips to required when the survived count reaches **0** for `crate::fs_guard` + `crate::detect`, P0.5.10). Release-tier — runs on the release-confirmation leg, never per-push (the run cost). (`needs: P0.5.10` — the P0-authored contract + ratchet shape, satisfied since P0 is `[x]` before the loop reaches P3; the kernel-body boxes P3.6/P3.8/P3.18/P3.29/P3.46/P3.68/P3.69 for the real bodies the mutants are injected into, mirroring the P3.67→P0.5.8 activation pattern.)
