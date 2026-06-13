# 02 — Guarantees (implementation of the SSOT hard promises)

> Each load-bearing SSOT guarantee gets a concrete technical implementation here.
> Origin: SSOT *Never harm the original*, *Fail clearly*, *Local/private/offline*,
> *Security posture*. The SSOT states the promise; this file states the mechanism.
>
> **Conventions.** Decision tags `[DECIDED]` / `[OPEN]` / `[DEFER]` per the spec
> [README](README.md). Rust identifiers/crates are named concretely so Phase 3 can
> be derived directly; where two implementations are equally valid a **recommended
> default** is marked and the genuine owner-level choice is flagged `[OPEN]` (it
> feeds the README open-questions log). **OS primitives are named per platform
> (Win / macOS / Linux) wherever they differ.**
>
> **What this file owns vs references.** This file owns the *guarantee mechanisms*:
> atomic write, no-clobber, resolved-identity link safety, the frozen set, re-run
> equivalence detection, cleanup/temp ownership, output destination + fallback, the
> **error taxonomy and its message catalog**, the **lossy-disclosure string
> catalog**, i18n filename/content handling, the privacy/offline invariants, and
> (as single owner) **decoder isolation (§2.12)**, the **app-level fault model
> (§2.13)** and the **temp/scratch + cross-volume strategy (§2.14)**. It does **not**
> own: the IPC contract (→ §0.4), the pipeline/queue/job lifecycle (→ §1.x), the
> per-format engine details and lossy *flags* (→ 04-formats), engine invocation
> lifecycle and cancellation (→ §1.7), per-engine argument construction (→ §3.5),
> instance/run-identity (→ §7.1), UI-chrome strings and surfacing (→ §5.7).

---

## 2.0 The reusable guarantees-fs layer (where this all lives) `[DECIDED]`

All mechanisms below are implemented **once**, in the **orchestrator / guarantees-fs
module** owned by §0.7 (not duplicated per engine or per format). Logical home
(name is illustrative; physical tree is §0.7's call):

- `core::fs_guard` — atomic write, no-clobber, resolved-identity, path-limit checks
  (§2.1 / §2.2 / §2.3 / §2.14).
- `core::run` — per-run/instance scratch ownership and cleanup (§2.6), keyed on the
  `RunId`/`InstanceId` defined by §7.1.
- `core::outcome` — the error taxonomy + message catalog (§2.8) and lossy catalog
  (§2.9), the **single source of every conversion-outcome string**.
- `core::isolation` — the decoder-isolation wrapper (§2.12) every engine spawn
  routes through (§1.7 calls it; §3.5 builds the args inside it).

The pipeline (§1.8 output planning, §1.7 invocation, §1.9 lifecycle) **calls into**
this layer; the layer never calls back up. Dependency direction: `fs_guard`,
`run`, `outcome`, `isolation` are leaf modules with no dependency on UI, IPC, or the
engine registry — they are the trust kernel that keeps the SSOT promises regardless
of which engine or format is in play.

---

## 2.1 No-clobber & atomic write `[DECIDED]`

**Promise (SSOT *Never harm the original*).** A conversion *either fully succeeds
or leaves no file behind*; the visible output appears **atomically**; a crash /
power loss / force-quit never leaves a truncated file masquerading as finished; the
no-clobber guarantee is **absolute** and evaluated on the **resolved real file, not
the path string**.

### 2.1.1 The write sequence (per output item)

The §2.1 atomic write **consumes the `OutputPlan`** produced by §1.8 (which already
applied the §2.7 destination rules). Given a *final resolved destination path*
`final` and a *resolved-equal* check from §2.3, the write is:

1. **Pick the scratch path** `tmp` on the **same volume as `final`** (§2.14 owns
   *which* volume and the cross-volume fallback). `tmp` is created in the per-run
   scratch dir (§2.6) with a unique, run-owned name, e.g.
   `convertia-<RunId>-<jobId>-<rand>.part`.
2. **Engine writes into `tmp`** (the engine is told to write to `tmp`, never to
   `final`; §3.5 constructs the arg). The engine runs through the §2.12 isolation
   wrapper.
3. On engine success: **`tmp.sync_all()`** (Rust `File::sync_all` → `fsync` on
   Unix, `FlushFileBuffers` on Windows) so the bytes are durable *before* the
   rename — per the durability research, atomic-name-update is **not** the same as
   durable-data.
4. **Resolve `final` and the no-clobber decision** (§2.2 numbering + §2.3 link
   safety) **as late as possible** — immediately before the create — to shrink the
   TOCTOU window.
5. **Exclusive create-or-fail of `final`**, then move the bytes in atomically. Two
   mechanically distinct steps because OS rename semantics differ (see 2.1.2).
6. **Durability of the rename:** on Unix, after the rename **fsync the containing
   directory** (open the parent dir, `fsync` its fd) so the new dentry survives a
   crash — per the LWN/evanjones durability findings (rename is atomic but not
   durable without the directory fsync). On Windows the directory-fsync step is a
   no-op (NTFS metadata journaling covers it); `MoveFileExW` with
   `MOVEFILE_WRITE_THROUGH` is the equivalent guarantee.
7. On engine failure / cancel / any error in steps 3–6: **`tmp` is removed**
   (§2.6); `final` was never created → nothing to undo. Cleanup failure is itself
   handled (§2.6: never reported as clean success).

### 2.1.2 Exclusive create + atomic publish — the OS-primitive split `[DECIDED]`

The **no-clobber** part and the **atomic-publish** part use *different* primitives
because no single cross-platform call does both (exclusive-create *and* fill-from-a
temp atomically). The chosen pattern:

- **Reserve the name exclusively first.** `OpenOptions::new().write(true)
  .create_new(true).open(final)` — Rust's `create_new` maps to **`O_CREAT|O_EXCL`
  on Unix** and **`CREATE_NEW` on Windows**. This is the OS-atomic
  *create-new-or-fail*: it fails with `ErrorKind::AlreadyExists` if a file **or a
  (dangling) symlink** already exists at `final`, closing the TOCTOU race the SSOT
  calls out ("even if the chosen name becomes taken between picking and writing").
  Per the std docs this is **the** race-free exclusive create on both OSes.
- **`AlreadyExists` → re-pick, never overwrite.** If the exclusive create fails
  with `AlreadyExists`, the no-clobber rule (§2.2) advances to the next free
  variant and retries — bounded retry loop (cap ~10 000 variants, then path-limit /
  too-many-collisions failure §2.8). This is what makes the guarantee **absolute
  against concurrent writers** (a second instance, a concurrent conversion, a file
  that appeared meanwhile): the *kernel* enforces "new or fail", not a prior
  `exists()` check.

The publish itself, given the reserved `final` handle, is **`[OPEN]` between two
mechanically-equal options** (recommended default below):

- **(a) Reserve-then-rename (recommended).** The exclusive `create_new` reserves
  the name as a 0-byte placeholder; we then `rename(tmp → final)` which **atomically
  replaces** the placeholder we own. On Unix `rename(2)` is atomic and replaces.
  On Windows plain `rename` **fails if the target exists**, so use
  **`MoveFileExW(tmp, final, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`**
  (exposed via the `windows` crate or `fs::rename` on modern Rust which already
  calls `MoveFileExW` with replace). The placeholder we created is *ours*, so
  replacing it cannot clobber anyone else's file.
- **(b) Write-into-the-reserved-handle.** Stream the engine output through the open
  exclusive handle directly (no temp + rename). **Rejected for the engine path**:
  engines are *separate processes* writing their own file (§3.5) — they cannot
  share our Rust file handle, and they may write non-atomically. (b) is only viable
  for in-core writes, which ConvertIA has none of (every output is engine-produced).

> **Recommendation:** **(a) reserve-then-`tmp`-rename**, because (1) it keeps the
> no-clobber reservation race-free via `create_new`, (2) the engine writes to a
> private `tmp` it fully controls, and (3) the final flip is a single atomic rename
> the OS guarantees. The placeholder is created with `create_new` and immediately
> closed; the window between reservation and rename is sub-millisecond and the
> rename is replace-our-own, so no third party can be clobbered. **Marked `[OPEN]`**
> only because the precise Windows replace-semantics ordering (placeholder vs
> rename-with-REPLACE_EXISTING) wants a small spike to confirm no race on NTFS; the
> *guarantee* is fixed, the *micro-mechanics* are the open detail. Owner: §2.1.

### 2.1.3 Crash / power-loss invariant `[DECIDED]`

After any ungraceful end, the on-disk state is exactly one of:

- **`final` exists and is complete** — the rename (step 5) committed; `sync_all`
  (step 3) + dir-fsync (step 6) guarantee its bytes are durable. *Success.*
- **`final` does not exist, a `*.part` temp may remain** — the rename had not yet
  committed. The temp is a **discardable run-owned artifact**, cleaned on next run
  (§2.6). *No partial output masquerading as finished.*

There is **never** a third state (a truncated `final`) because the engine never
writes to `final` and the publish is a single atomic rename. This satisfies the
SSOT "holds even across an ungraceful end". Cross-volume nuance (when `tmp` and
`final` cannot be on the same volume) is §2.14.

---

## 2.2 Output naming contract `[DECIDED]`

**Promise (SSOT *Never harm the original*).** Output **keeps the source's base name
and takes the target format's extension** (`vacation.heic` → `vacation.jpg`).
No-clobber numbering appends `(1)`, `(2)`… **before the extension**; the base name
is **never** replaced, hashed, or decorated (`_converted` etc.). A name whose suffix
or new extension would exceed the OS path limit **fails clearly** (no truncation).

### 2.2.1 Name construction

Given source `base.srcext` and a target extension `tgtext` (from 04-formats), the
output name is computed by `fs_guard::output_name`:

```
stem      = source file stem, preserving the exact Unicode bytes (§2.10)
ext        = target's canonical extension (lowercase, e.g. "jpg", "mp4", "m4a")
candidate  = format!("{stem}.{ext}")              // first attempt
on collision: format!("{stem} ({n}).{ext}")        // n = 1,2,3,…
```

- The **space-paren** form `stem (1).ext` is the SSOT-mandated shape (a space then
  `(n)`), matching the OS-native "next copy" convention users recognise. It is
  **not** `stem_1`, `stem-1`, or a hash.
- The **stem is taken verbatim** — multi-dot names (`my.report.final` →
  `my.report.final.pdf`), names that are *only* an extension-looking token, and the
  same-format case (`photo.jpg` → re-encode → `photo (1).jpg`, never overwriting the
  source) all preserve the full original stem (§2.10 handles the bytes).
- Extension is the **target's** canonical extension regardless of the source's true
  vs claimed extension (a misnamed `.jpg`-that-is-PNG converted to WEBP →
  `name.webp`).

### 2.2.2 Collision discovery is via §2.1's exclusive create, not a pre-scan

`n` is **not** chosen by listing the directory and picking max+1 (that is itself a
TOCTOU race). Instead `output_name` produces candidates **lazily** and each
candidate is handed to §2.1.2's `create_new`; on `AlreadyExists` it yields the next
candidate. So numbering and the absolute no-clobber guarantee are the **same loop** —
the directory's real state at the instant of create decides, not a stale scan. (An
optional cheap `symlink_metadata` pre-check may skip obviously-taken low numbers as
an optimisation, but the **authority is always the exclusive create**.)

This is the technical realisation of the SSOT distinction:

- **Ordinary collision** (an unrelated pre-existing file, or within-run) → silent
  next-free-variant numbering (this loop). No prompt.
- **Re-run of the identical conversion** → handled *before* this loop by §2.5
  (one batch-level prompt). §2.5's equivalence check runs first; only if it does
  **not** fire do we fall through to silent numbering.

### 2.2.3 Path-limit handling (fail, never truncate) `[DECIDED]`

Before attempting the exclusive create, `fs_guard::check_path_limit(final)`
validates the **resolved final path length** against the OS limit:

- **Windows:** classic `MAX_PATH` = **260** chars for the full path (drive + dirs +
  name + NUL). ConvertIA's portable build does **not** assume the "long path aware"
  manifest/registry opt-in is present on the user's machine (it is not portable to
  rely on it), so the conservative ceiling is `MAX_PATH`. **Mitigation:** internally
  all FS calls use the **extended-length `\\?\` prefix** (via the `dunce` crate's
  inverse — we *add* `\\?\` for our own syscalls, see §2.3.4) so ConvertIA itself
  can read/write long paths the engines were handed; but a **final output path that
  the user/Explorer cannot then open** is still surfaced as a failure rather than a
  silent success. The check is: would the *user-facing* (non-`\\?\`) form exceed
  260? → fail clearly. Individual path **component** limit is **255** UTF-16 code
  units (NTFS) — also checked.
- **macOS (APFS/HFS+):** per-component limit **255 UTF-8 bytes** (NFC/NFD nuance,
  §2.10); total path is effectively bounded by `PATH_MAX` (1024) for many APIs.
- **Linux:** per-component **255 bytes** (`NAME_MAX`), total **4096** (`PATH_MAX`).

When appending `(n)` or swapping the extension would push the name past the
**component** limit or the path past the **total** limit, ConvertIA emits the
`PathTooLong` failure (§2.8) — **truncation is never the escape hatch** (SSOT). The
check runs on the **fully-resolved** path including any §2.7 divert, so the
divert-path enjoys the identical guarantee (SSOT: "apply identically on the
divert/fallback path").

---

## 2.3 Resolved-identity & link safety `[DECIDED]`

**Promise (SSOT *Never harm the original*).** ConvertIA never writes to, through, or
as a target that resolves (via **symlink, alias, junction or hardlink**) onto any
source in the frozen set; if writing beside a source would resolve onto the
original, it **diverts** (§2.7) rather than risk it. The frozen set is
**de-duplicated by resolved identity** (a file reached via two paths is converted
once). No-clobber is evaluated on the **resolved real file**.

### 2.3.1 Canonical identity of a path `[DECIDED]`

Every source and every candidate output path is reduced to a **canonical identity**
by `fs_guard::resolve_identity(path) -> FileIdentity`:

- Primary: **`std::fs::canonicalize`** (Unix `realpath`-equivalent; resolves all
  symlinks and `.`/`..`). On Windows `canonicalize` returns a **verbatim `\\?\`
  UNC** path; we normalise the *display/comparison* form with **`dunce::canonicalize`**
  (returns the most-compatible non-UNC form when possible) so two paths that differ
  only by `\\?\` prefix compare equal.
- For **identity comparison** (the load-bearing "same file?" test) ConvertIA does
  **not** rely on string equality of canonical paths alone — it also compares the
  **OS file identity**:
  - **Unix:** `(st_dev, st_ino)` from `fs::metadata` (`MetadataExt::dev` /
    `MetadataExt::ino`). Equal `(dev, ino)` ⇒ the **same inode** ⇒ catches
    **hardlinks** (which `canonicalize` cannot, since hardlinks share no link to
    follow — two distinct paths, one inode).
  - **Windows:** the **`(volumeSerialNumber, fileIndexHigh, fileIndexLow)`** from
    `GetFileInformationByHandle` (via `std::os::windows::fs::MetadataExt`
    `volume_serial_number()` / `file_index()`, available on recent Rust, else the
    `windows` crate). Equal triple ⇒ same file ⇒ catches **hardlinks** and
    **junctions** that point at the same backing file.
  - **macOS:** same `(st_dev, st_ino)` as Unix; **Finder aliases** (the classic
    `.alias` bookmark) are *data files*, not filesystem links — they are **not**
    transparently followed by `canonicalize`, so an alias dropped as a source is
    detected as its own (alias-document) type and never confused with its target.
    **Symlinks** and **hardlinks** on macOS behave as Unix.

`FileIdentity` therefore = `{ canonical_path, dev_or_volserial, inode_or_fileindex }`.
Two paths are the **same resolved file** iff the device+inode identity matches
(authoritative), with the canonical path as a fast pre-filter.

### 2.3.2 De-duplicating the frozen set `[DECIDED]`

When the frozen set is built (§2.4, §1.1), each entry is keyed by `FileIdentity`.
Two dropped paths that resolve to the same inode/file-index (a symlink + its target
both dropped; a folder containing both a file and a hardlink to it) collapse to
**one** `DroppedItem` → **converted once** (SSOT). The retained representative path
is the **first-seen** path (deterministic), but identity — not the path string — is
the dedup key.

### 2.3.3 Write-target safety check `[DECIDED]`

Before §2.1's exclusive create, `fs_guard::is_safe_output(final, frozen_set)`:

1. Compute `resolve_identity(final)`. If `final` does **not** exist yet (the normal
   case), resolve **its parent directory's** identity and the *would-be* path; a
   non-existent leaf cannot itself be a link, but its **parent** can be a symlink
   into a source tree — so the parent is canonicalised and the leaf appended.
2. **Reject if** the resolved `final` (or its resolved parent + leaf) has a
   `FileIdentity` equal to **any** source in the frozen set, **or** lands *inside* a
   directory that is itself a source (folder-drop) by canonical-prefix containment.
   "Writing beside a source would resolve onto the original" is exactly this case
   (e.g. the output dir is a symlink back into the source dir).
3. On reject → **divert** to the §2.7 per-location fallback (Downloads/Documents or
   user-chosen), **never** proceed. The divert path is then re-checked (it too must
   pass `is_safe_output`).

Because step 2 also runs as part of the §2.1 `create_new` loop, a link that is
created *between* the check and the write still cannot clobber a source: `create_new`
fails `AlreadyExists` on the existing (symlink) target and we re-pick.

### 2.3.4 Per-OS link primitives (named) `[DECIDED]`

| Link kind | Win | macOS | Linux | Detected by | Followed by `canonicalize`? |
|-----------|-----|-------|-------|-------------|------------------------------|
| **Symlink** | symbolic link (`mklink`, requires privilege/Dev-Mode) | symlink | symlink | `symlink_metadata().file_type().is_symlink()` | yes (resolved) |
| **Junction** | NTFS reparse point (dir-only) | — | — | reparse-point attr via `MetadataExt::file_attributes()` `FILE_ATTRIBUTE_REPARSE_POINT` | partially — handled via file-index identity (§2.3.1) |
| **Hardlink** | NTFS hardlink | hardlink | hardlink | `nlink > 1` + identity triple/inode | **no** (no link to follow) → caught by dev+inode / file-index |
| **Alias** (macOS) | — | Finder bookmark **data file** | — | content sniff (§1.2) → treated as its own document type | n/a (not an FS link) |

The two that `canonicalize` alone **misses** — hardlinks (everywhere) and junctions
(Windows) — are exactly why the dev+inode / volume-serial+file-index identity check
is mandatory, not optional. ConvertIA does **not** itself create any symlinks/
hardlinks/junctions (it only writes plain files), so it only has to *detect*, never
*author*, these.

---

## 2.4 Frozen source set & no self-feeding `[DECIDED]`

**Promise (SSOT *Never harm the original* / *How It Feels*).** The source set is
**frozen at the moment of drop/selection**; any file appearing afterward — written
by this run, a concurrent instance, anything — is **never ingested** as a source in
this run; outputs landing in a source folder do **not** expand or restart the batch.

### 2.4.1 The freeze `[DECIDED]`

§1.1 (intake) is the **single funnel** for every entry point (native file-drop,
picker, keyboard, OS launch args/open-with). It builds the frozen set **eagerly and
once**, *before* any conversion starts:

- A dropped **folder is fully enumerated recursively in Rust** at freeze time (§0.4
  boundary fact: the WebView cannot enumerate directories; §1.1 owns the walk) —
  the recursion result is materialised into a concrete `Vec<DroppedItem>` snapshot.
  Hidden/system files (`.DS_Store`, `Thumbs.db`, dotfiles per the §1.1 ignore rule)
  are filtered **at freeze time**.
- Each entry is reduced to a `FileIdentity` (§2.3) and **de-duplicated** (§2.3.2).
- The snapshot is **immutable** for the run. Conversion iterates the snapshot; it
  never re-reads the directory. This is what makes "outputs landing in a source
  folder do not expand the batch" a *structural* property, not a guard: the walk
  already happened and produced a fixed list; new files in that folder are simply
  not in the list.

### 2.4.2 No self-feeding — three structural defences `[DECIDED]`

1. **Snapshot, not live iteration** (above): a freshly-written output in a source
   folder is invisible to this run because the list was frozen pre-run.
2. **Resolved-identity dedup** (§2.3.2): even if an output path *coincidentally*
   equals a frozen source's resolved identity, §2.3.3 diverts the write rather than
   producing it there.
3. **Run-owned temp namespace** (§2.6): in-progress `*.part` artifacts are named
   with the `RunId` and live in the per-run scratch dir, so they could never be
   mistaken for a droppable source even by a *different* concurrent instance's walk.

### 2.4.3 Concurrent-instance & launch hand-off `[DECIDED]`

The freeze point is **exhaustive including the second-instance / launch hand-off**
(§7.1 / §7.8 own the instance policy). Whatever instance policy §7.1 picks
(single-instance forwarding launch args to the running instance, or independent
instances), the rule here is: **each batch's frozen set is captured at the instant
that batch is created**, and a *later* drop (even into the same window) starts a
*new* frozen set / batch — it never mutates an in-flight one. Files produced by a
concurrent instance are foreign and, being absent from this run's snapshot, are
never ingested (SSOT: "a concurrent instance ... never ingested as a source in this
run").

---

## 2.5 Re-run / equivalent-output detection `[DECIDED, best-effort]`

**Promise (SSOT *Never harm the original*).** When ConvertIA detects it would
re-produce output for the **same resolved source + same target + same effective
settings**, it does **not** silently add another numbered copy — it shows **one
plain batch-level prompt** (skip = safe default, or make a fresh copy). Any change
to target or settings is a new conversion using ordinary numbering. Detection is
**best-effort**: when it can't tell (a prior output was renamed/moved, or across
sessions) it safely falls back to **silent next-free-variant numbering, never to
overwriting**.

### 2.5.1 The equivalence key `[DECIDED]`

```
EquivKey = hash(
    source_identity,           // FileIdentity (§2.3) — resolved, not the path string
    target_format,             // e.g. "webp"
    effective_settings_canon,  // canonicalised settings struct for THIS pair
)
```

- `effective_settings_canon` is the **fully-defaulted** option set for the pair
  (the §1.6 no-decision defaults *merged with* any user overrides), serialised in a
  **stable, order-independent** form (sorted keys; normalised numeric/enum values)
  so that "left everything default" twice produces the **same** key. The option
  model is owned by §1.6 / 04-formats; §2.5 only consumes the resolved values.
- Source **identity** (not path) means a re-run reached via a different but
  same-file path still matches.

### 2.5.2 Detecting "this exact conversion already produced output" `[DECIDED]`

Detection is **best-effort** and uses two cheap, offline signals (no DB, honoring
§7.4's "persist nothing / session-only" lean — see *fallback* for the cross-session
limit):

1. **Expected-output presence.** Compute the **first** candidate output name
   (`stem.ext`, §2.2) at the planned destination (§2.7). If a file already exists
   *there* whose presence is consistent with a prior identical run, that is the
   strong signal. Because ConvertIA writes **deterministic** names, the prior run's
   output is exactly where this run's first candidate would go.
2. **In-session run ledger (recommended).** Within the **current app session**,
   `core::run` keeps an in-memory `HashSet<EquivKey>` of conversions already
   completed this session (cleared on quit; nothing written to disk, §7.4). A second
   identical drop in the same session hits the ledger → definite equivalence.

A batch-level prompt fires when **any** item in the batch is flagged equivalent.
It is **one** prompt for the whole batch (SSOT: "one plain batch-level prompt"),
surfaced as the §5.2 *re-run prompt* state with `Skip` (default, focused) /
`Make fresh copies`. The prompt's strings are UI-chrome (§5.7), but the **decision
semantics** (skip-default; fresh-copy → ordinary numbering) are owned here.

### 2.5.3 Best-effort fallback (never overwrite) `[DECIDED]`

When ConvertIA **cannot** determine equivalence — the prior output was **renamed or
moved** (so the deterministic name is free again), or this is a **new session** and
the ledger is empty, or the destination differs — it **does not** guess. It falls
through to §2.2 **silent next-free-variant numbering**. The invariant the SSOT
pins: the *failure mode of detection is a harmless extra numbered copy, never an
overwrite*. This is acceptable precisely because §2.1's exclusive-create makes
overwrite impossible regardless of what §2.5 concludes — §2.5 only decides *prompt
vs silent-number*, never *number vs overwrite*.

> `[OPEN]` (owner §7.4 / §2.5): whether a **cross-session** ledger is worth adding
> (a tiny on-disk record of completed `EquivKey`s) to make re-run detection survive
> a restart. SSOT leans "persist nothing"; the privacy invariant (§2.11) would
> require the ledger to store **only opaque hashes, never paths/content**.
> **Recommendation: do not persist in v1** — keep §2.5 session-only; the harmless
> extra copy across sessions is within the SSOT's stated best-effort tolerance.
> Flagged because it is genuinely a product/persistence call, not a mechanism call.

---

## 2.6 Cleanup, temp ownership & free-space restoration `[DECIDED]`

**Promise (SSOT *Never harm the original* / *Fail clearly* / *How It Feels*).**
Partial/temporary artifacts are removed on failure / cancel / out-of-disk so free
space returns to roughly pre-run; temp artifacts are **owned per-run** so cleanup
never removes another instance's in-progress file; startup cleanup removes
*discardable* temps from prior crashed runs; if cleanup itself can't complete, the
item is **never reported as a clean success** — ConvertIA says residue may remain
and where.

### 2.6.1 Temp ownership model `[DECIDED]`

- **Scratch root** is owned by §2.14 (where it lives, which volume). Within it, each
  run gets a **run-scoped subdirectory** named with the `RunId` (§7.1):
  `…/convertia/run-<RunId>/`. Every `*.part` (§2.1) and any engine working file
  (§3.5 working dir) lives under that run dir.
- **`RunId` encodes ownership**, so a cleanup sweep can tell *its own* temps from a
  *concurrent instance's* temps. The RunId model and its uniqueness/liveness
  semantics are §7.1's to define; §2.6 *requires* it to be (a) unique per run and
  (b) liveness-checkable (so a stale dir from a dead run is distinguishable from a
  live one — see 2.6.3).

### 2.6.2 Cleanup triggers `[DECIDED]`

`core::run::cleanup_item` / `cleanup_run` remove run-owned temps on every exit path:

| Trigger | Action |
|---------|--------|
| **Item success** | `tmp` already renamed to `final` (§2.1); nothing to remove. |
| **Item failure** (engine error, corrupt, etc.) | remove that item's `tmp`. |
| **Cancel** (user) | §1.7 kills the engine; the killed item's `tmp` is removed; **already-finished items are kept** (SSOT). |
| **Out-of-disk mid-write** | remove the partial `tmp`; report `OutOfDisk` (§2.8); **batch continues** (SSOT). |
| **Run end (any reason)** | remove the now-empty `run-<RunId>/` dir. |
| **Next app start** | sweep stale `run-<RunId>/` dirs from prior runs (§2.6.3). |

Removal restores free space to "roughly what it was before the run" (SSOT) — temps
are the only thing ConvertIA adds to disk besides the final outputs, and successful
finals are intended; failed/cancelled items leave nothing.

### 2.6.3 Startup sweep — never touch a live instance's temp `[DECIDED]`

On startup (§7.2 sequence) `core::run::sweep_stale`:

1. Lists `convertia/run-*` dirs under the scratch root.
2. For each, checks **liveness** via §7.1's mechanism — recommended: an **advisory
   lock file** `run-<RunId>/.lock` held with an OS lock for the run's lifetime
   (Unix `flock`/`fcntl` `F_SETLK`; Windows `LockFileEx` exclusive on the lock
   file). A dir whose lock is **still held** belongs to a **live** instance → **left
   untouched**. A dir whose lock is **free/stale** belongs to a dead/crashed run →
   removed (its `*.part` files are the discardable artifacts the SSOT says are
   "cleaned up on next run").
3. This is what makes the SSOT promise *"temp artifacts are owned per-run so cleanup
   never removes another instance's in-progress file"* concrete: liveness is by
   held-lock, not by guessing from timestamps.

### 2.6.4 Cleanup failure → honest reporting `[DECIDED]`

If removing a temp **fails** (a lock held by AV software, a read-only scratch that
went away, permission flip), the item is **not** silently downgraded. Two cases:

- **The output succeeded but its `tmp` couldn't be removed** (rare — `tmp` is
  normally renamed, not deleted): the success stands, but the §1.12 summary carries
  a `residue` annotation: *"converted — a temporary file may remain at &lt;path&gt;"*.
- **An item failed *and* its partial couldn't be cleaned**: the item is reported as
  **failed** (§2.8) **with** the `CleanupResidue` annotation naming the path (SSOT:
  "ConvertIA says residue may remain and where"). It is **never** counted as a clean
  success. The string lives in the §2.8 catalog (`cleanup_residue` row).

---

## 2.7 Output destination & per-location fallback `[DECIDED]`

**Promise (SSOT *How It Feels* 7 / *Never harm the original*).** Destination is
**shown and changeable before** convert (the "will save to…" line); default is
**beside each source in place** (folder layout preserved naturally); a user-chosen
destination **re-creates the relative subfolder structure** (not flattened). The
fallback is **per-location**: a source whose location can't be written (read-only
USB, network share, restricted folder) — or that sits in a **known-ephemeral** place
(a temp dir) — **diverts** to a single predictable place (Downloads/Documents or a
user-picked folder), while writable sources still get output beside them. Flattened
fallback outputs are still de-collided by no-clobber; the summary maps each output
to its source; "open folder" opens the **common root**. All guarantees hold on the
divert path.

> **Ownership note.** §2.7 owns the **rules**; §1.8 owns *computing* the
> `OutputPlan` by applying these rules before the write; §2.1 *consumes* the plan;
> §5.2/§5.3 *show* the "will save to…" line and the destination chooser. §2.7 does
> not own the UI or the pipeline step — only what-the-rules-are.

### 2.7.1 Destination modes `[DECIDED]`

1. **Beside source (default).** Output goes in the **same directory as the source**.
   Folder layout is preserved for free (each output sits next to its origin). This
   is the SSOT default and needs no path computation beyond the source's parent.
2. **User-chosen destination.** A single chosen root `D`. For a source at relative
   path `sub/dir/file.ext` *within the dropped selection's common root*, the output
   is written to `D/sub/dir/file.<tgt>` — the **relative subtree is re-created**
   under `D` (SSOT: "re-creates the relative subfolder structure rather than
   flattening"). The common root is the deepest directory containing all frozen
   sources (computed at freeze, §2.4).

### 2.7.2 Per-location writability & ephemerality classification `[DECIDED]`

For each source, §1.8 classifies its **intended** output location via
`fs_guard::location_status(dir)`:

- **Writable test:** attempt to create (and immediately remove) a probe file via
  `create_new` in the target dir (the same primitive as §2.1, so the test matches
  reality). Failure (`PermissionDenied`, `ReadOnlyFilesystem`, network errors) →
  **unwritable**. *Recommended:* probe lazily and cache per-directory within the run
  to avoid probing every file in a 10 000-file batch in the same folder.
- **Ephemeral test:** is the dir inside a **known-ephemeral OS temp location**?
  - Win: under `%TEMP%` / `%TMP%` / `GetTempPathW`.
  - macOS: under `$TMPDIR` (per-user `…/T/`), `/tmp`, `/var/folders/…`.
  - Linux: `$TMPDIR`, `/tmp`, `/var/tmp`, `/run/user/<uid>` (XDG runtime).
  Writing a *result* into a place the OS may purge silently loses the user's output
  → treated like unwritable → divert. (Reading a source from there is fine; only the
  *output* diverts.)
- A **read-only USB / network share** surfaces as unwritable by the probe.

### 2.7.3 Divert target `[DECIDED]`

When a location is unwritable/ephemeral, that source's output **diverts** to a
single predictable place (per-location, not whole-batch):

- **Default divert root:** the user's **Downloads** dir, falling back to
  **Documents** if Downloads is absent — resolved via Tauri v2's `PathResolver`
  (`download_dir()` / `document_dir()`) so it is correct per-OS and localised. The
  divert root may be **overridden** by the user-chosen destination from §2.7.1
  (the chooser doubles as the divert target).
- **Mixed batch:** writable sources still write **beside** themselves; only the
  unwritable/ephemeral ones divert. This is the SSOT "per-location" semantics — the
  divert is item-by-item, never an all-or-nothing whole-batch redirect.

### 2.7.4 Flattening, de-collision, and the summary `[DECIDED]`

- Diverted outputs from different source subtrees can **collide by name** in the
  single divert root. They are **de-collided by the §2.2 no-clobber numbering**
  exactly as anywhere else (SSOT: "Flattened fallback outputs are still de-collided
  by the no-clobber rule"). The divert path uses the same §2.1 exclusive-create
  loop — **all no-harm / atomicity / path-limit / free-space guarantees apply
  identically** (SSOT, explicit).
- The §1.12 **completion summary maps each output back to its source** (so a
  flattened `report (3).pdf` is traceable to which `report` it came from). §2.7
  requires the summary to carry `source → output` pairs; §1.12 owns the structure.
- **"Open folder"** opens the **common root of the dropped selection** for the
  beside-source case, and the **divert root** for diverted items — the SSOT says
  open-folder opens the common root; where outputs were split (some beside, some
  diverted) the summary's per-item "open file/folder" (§5.3 OpenActions, §7.7
  shell-out) reaches each one. *Recommended:* the primary "open folder" button opens
  the common root; per-item rows offer "open containing folder" for diverted items.

### 2.7.5 Guarantees on the divert path `[DECIDED]`

Restating the SSOT explicitly: the **divert/fallback path is not a degraded path**.
The §2.3 link-safety check, §2.1 atomic write + exclusive create, §2.2 naming +
path-limit, §2.5 re-run detection, §2.6 cleanup, and §2.10 i18n handling **all run
identically** on a diverted output. There is no code path where a divert skips a
guarantee.

---

## 2.8 Error taxonomy & fail-clearly — **the message catalog (home)** `[DECIDED]`

**Promise (SSOT *Fail clearly, never cryptically*).** A corrupt / empty / 0-byte /
unrecognizable / out-of-scope file — or a source unreadable-or-gone when its turn
comes — produces **one plain-language message** and nothing written; the **rest of a
valid batch keeps going** (a bad item is skipped mid-run and reported, never
silently); out-of-disk / too-big fails clearly **and** the batch continues; a batch
where *everything* failed is a **clear failure**, never a quiet finish. **No stack
traces.**

> **Ownership.** §2.8 is the **single home of every conversion-outcome failure
> string** (this section) plus §2.9 (lossy strings). §1.7 maps engine exit/timeout
> to these kinds; §1.9 drives batch-continue; §1.12 assembles the summary; §5.7
> *surfaces* the strings. UI-chrome strings (empty-state, buttons, confirm-gate,
> About) are §5's and share the same future-localization boundary. §2.8 produces
> **machine-stable kinds + the canonical English string**; the WebView renders them.

### 2.8.1 The `ConversionError` taxonomy `[DECIDED]`

A Rust enum in `core::outcome`, each variant a **stable kind** carried over IPC
(§0.4 owns the wire shape; §2.8 owns the *set* and their strings). Every engine /
FS / detection failure **must** map to exactly one of these — there is no "other /
unknown" that leaks a raw error to the user (an unmapped internal error becomes
`InternalError` with a generic calm message, §2.13).

```rust
enum ConversionErrorKind {
    // ── item-level (one source file failed; the batch continues §1.9) ──
    Corrupt,            // decoded but structurally invalid / truncated mid-stream
    Empty,              // 0-byte or no decodable content
    Unrecognized,       // detection cannot identify the type at all (§1.2 uncertain/conflicting)
    UnsupportedType,    // recognised but not an in-scope source (§1.2 "detected: X")
    UnsupportedPair,    // in-scope source, but target not offered (defensive; UI prevents)
    Unreadable,         // present at freeze, now unreadable: perm denied / exclusive lock
    Gone,               // present at freeze, now missing: moved/deleted/removed media
    PasswordProtected,  // encrypted/DRM source (PDF pw, FairPlay, PlaysForSure) — see 04
    NoAudioTrack,       // extract-audio asked of a source with no audio stream (cross-cat / audio.md)
    TooBig,             // exceeds the §1.10 "too big" ceiling (pre-flight or mid-run)
    OutOfDisk,          // ENOSPC while writing (§2.6 cleans the partial)
    WriteFailed,        // the output write/publish failed for a non-space reason (perm/IO at the destination, §2.1/§2.7)
    PathTooLong,        // §2.2.3 — name/extension would exceed OS path limit
    EngineCrash,        // subprocess killed by signal / nonzero abnormal exit (§1.7/§2.12)
    EngineHang,         // exceeded the §1.7 timeout, killed (§2.12)
    EngineError,        // subprocess clean nonzero exit w/ classifiable stderr (§3.5)
    PlatformUnavailable,// patent-gapped on this platform (§3.4) — honest "unavailable here"
    CleanupResidue,     // item failed AND its partial couldn't be removed (§2.6.4)
    InternalError,      // catch-all for an unexpected internal fault (§2.13), no trace shown
    // ── run/app-level (§2.13); surfaced via app://fault, not a per-item row ──
    MixedDrop,          // >1 source format in one drop — pre-flight refusal (§1.3); chrome string §5
    EngineMissing,      // a required bundled engine is absent/unrunnable at startup (§7.2)
    WebviewFault,       // the WebView core disconnected / failed to load (§2.13/§5.8)
    BundleDamaged,      // the app bundle/resources failed their integrity check (§7.2)
}
```

A `ConversionError` carries the kind, the **owning source path** (for the summary),
optional **detected-type detail** (for `UnsupportedType`), and an optional
**residue path** (for `CleanupResidue`). It deliberately carries **no** stack trace,
no Rust `Debug` of the underlying error, no engine command line (that goes to the
local log §7.5 if enabled, never to the user — SSOT "no stack traces").

The **item-level** kinds are reported as a per-item `Failed` row and the batch
keeps going (§1.9); the **run/app-level** kinds (`MixedDrop`, `EngineMissing`,
`WebviewFault`, `BundleDamaged`) are not per-item outcomes — they travel over the
`app://fault` / refusal path (§0.4.2, §2.13) and `MixedDrop` specifically is the
pre-flight refusal (§1.3), surfaced with §5 chrome (the catalog below covers the
item-level kinds; the app-level kinds carry §5/§7.2 chrome strings, not §2.8.2
rows).

### 2.8.2 The message catalog `[DECIDED]`

The **exact canonical English strings**. One row per kind. `{x}` are runtime
substitutions filled by `core::outcome` (the type name, the path, the size). Tone:
plain, calm, never blaming, never technical (SSOT *Fail clearly*). These are the
**conversion-outcome** strings; UI-chrome strings live in §5.

| Kind | Canonical English message | Substitutions | Notes |
|------|---------------------------|---------------|-------|
| `Corrupt` | **"This file looks damaged and couldn't be converted."** | — | corrupt/truncated; per-format detail may append, e.g. images "the image data is incomplete". |
| `Empty` | **"This file is empty — there's nothing to convert."** | — | 0-byte or no decodable content. |
| `Unrecognized` | **"ConvertIA couldn't tell what kind of file this is, so it can't convert it."** | — | detection gave no confident type (§1.2 uncertain/conflicting). |
| `UnsupportedType` | **"ConvertIA can't convert this type of file — it looks like {detected}."** | `{detected}` = friendly type name | the SSOT "detected: X" case; e.g. "it looks like a ZIP archive." |
| `UnsupportedPair` | **"That conversion isn't available."** | — | defensive only; the UI never offers an unavailable pair. |
| `Unreadable` | **"ConvertIA couldn't open this file — it may be in use by another program, or you don't have permission to read it."** | — | exclusive lock / EACCES; was present at freeze. |
| `Gone` | **"This file is no longer there — it may have been moved, renamed, or its drive removed."** | — | present at freeze, missing at its turn (removable media, etc.). |
| `PasswordProtected` | **"This file is password-protected or copy-protected, so ConvertIA can't read it."** | — | encrypted PDF, DRM video/audio. ConvertIA never prompts for / cracks passwords. |
| `NoAudioTrack` | **"This file has no audio to extract."** | — | extract-audio asked of a video/source with no audio stream (cross-category.md / audio.md). |
| `TooBig` | **"This file is too large for ConvertIA to convert on this computer."** | — | §1.10 ceiling; for to-GIF the friendlier 04 variant ("too long/large to turn into a GIF — try a shorter selection") overrides via detail. |
| `OutOfDisk` | **"There isn't enough free disk space to finish this conversion."** | — | batch continues; partial cleaned (§2.6). |
| `WriteFailed` | **"ConvertIA couldn't save the converted file to that location."** | — | non-space write/publish failure at the destination (permission/IO, §2.1/§2.7); distinct from `OutOfDisk`. |
| `PathTooLong` | **"The output name would be too long for this system, so this file was skipped. Try a shorter folder or file name."** | — | never truncates (§2.2.3). |
| `EngineCrash` | **"Something went wrong while converting this file, so it was skipped."** | — | subprocess crash; no trace shown. Detail goes to §7.5 log only. |
| `EngineHang` | **"This file took too long to convert and was stopped."** | — | §1.7 timeout. |
| `EngineError` | **"ConvertIA couldn't convert this file."** | — | clean nonzero exit; generic calm fallback. |
| `PlatformUnavailable` | **"This conversion isn't available on {platform} because the required format support can't be included here."** | `{platform}` | the §3.4 honest per-platform gap; SSOT v1-DoD exception 1. |
| `CleanupResidue` | **"This file couldn't be converted, and a temporary file may remain at {path}."** | `{path}` | the only failure that names a path of residue (§2.6.4). |
| `InternalError` | **"Something unexpected went wrong, so this file was skipped. The rest of your files will continue."** | — | §2.13; never a stack trace. |

**Batch-level summary strings** (assembled by §1.12, strings owned here):

| Situation | Canonical English |
|-----------|-------------------|
| All succeeded | **"All {n} files converted."** |
| Partial | **"{ok} of {n} files converted. {fail} couldn't be converted — see details."** |
| All failed | **"None of the {n} files could be converted."** (an explicit failure, never a quiet finish — SSOT) |
| Cancelled | **"Stopped. {ok} files were already converted and kept; the rest were not started."** |
| With residue | append **"Some temporary files may remain — see details."** |

### 2.8.3 Behaviour rules tying the catalog to the pipeline `[DECIDED]`

- **One message per failed item** — never a cascade of dialogs; failures collect
  into the §1.12 summary, surfaced calmly (§5.7), never as a modal per file.
- **Batch continues** on every item-level kind above (§1.9 mid-run skip). The
  *pre-flight* mixed-format refusal (§1.3) is a different thing — a hard reject
  *before* converting — and uses §5's chrome strings, not this catalog (SSOT
  explicitly distinguishes the two).
- **Nothing written for a failed item** — guaranteed by §2.1 (the engine wrote only
  to `tmp`, removed on failure by §2.6).
- **No stack traces, ever** — `InternalError` is the floor; the underlying error's
  detail is logged locally only if §7.5 logging is enabled, with §7.5 redaction.

---

## 2.9 Lossy disclosure — **the lossy-note string catalog (home)** `[DECIDED]`

**Promise (SSOT *Fail clearly*).** Some conversions are inherently lossy; ConvertIA
signals predictable loss as a **calm, passive inline note next to the chosen
target** — shown **only** for genuinely predictable loss, **never** a blocking "I
understand" dialog or a per-conversion nag. This note is about **content
faithfulness, not downstream compatibility** (a valid WEBP/OPUS may not open
everywhere — that is the default-target tie-breaker's job, not a lossy note).

> **Ownership.** §2.9 is the **single home of every lossy-note string**. The
> 04-formats files record **which** (source,target) pairs are lossy (their `✓~`
> matrix flags) and **link here** — they never restate a string. §5.7 surfaces the
> note passively at target choice. The note is keyed by a **`LossyKind`**, so 04's
> flags map to a kind, and the kind maps to the one canonical string below.

### 2.9.1 `LossyKind` → canonical note (the catalog) `[DECIDED]`

The note is a **calm single line**. It appears once, next to the chosen target, the
moment a lossy target is selected (§5.7) — passive, dismissible-by-ignoring, never
gating the Convert button.

| `LossyKind` | Triggering pairs (from 04) | Canonical English note |
|-------------|----------------------------|------------------------|
| `image_lossy_codec` | `→ JPG/WEBP(lossy)/HEIC/AVIF` from any source (images.md) | **"Saved with compression — fine details may be slightly reduced."** |
| `image_palette` | `→ GIF` (256-colour); `→ ICO` (downscaled sizes) | **"Reduced to 256 colours — some colour detail is lost."** |
| `image_alpha_flatten` | alpha source `→ JPG/BMP` (transparency policy) | **"Transparency isn't supported here and will be filled with a background colour."** |
| `image_animation_flatten` | animated source `→` still target (animation policy) | **"Animated — only the first frame is converted."** |
| `image_svg_raster` | `SVG → raster` (svg entry) | **"Vector image converted to a fixed-size picture ({w}×{h}) — it won't scale up cleanly afterward."** |
| `doc_pdf_reflow` | `DOCX/DOC/ODT/RTF → PDF` (documents.md) | **"Layout may shift slightly when converted to PDF."** |
| `doc_pdf_to_text` | `PDF → TXT` | **"Text only — layout, tables and images are dropped."** |
| `doc_html_render` | `HTML → PDF` | **"The result may look different from a web browser."** |
| `doc_to_text` | `* → TXT` from rich sources | **"Text only — formatting and images are dropped."** |
| `doc_simplified` | `* → MD/RTF` from rich sources | **"Some formatting may be simplified."** |
| `sheet_to_delimited` | `XLSX/XLS/ODS → CSV/TSV` (spreadsheets.md) | **"Only one sheet and its values are exported — formatting, formulas and other sheets are dropped."** |
| `xls_legacy_limits` | `* → XLS` (spreadsheets.md) | **"Saved in the old Excel format — rows/columns beyond the legacy limit and newer features are dropped."** |
| `text_encoding_narrowed` | `CSV/TSV → workbook/CSV` with a non-Unicode chosen encoding (spreadsheets.md) | **"Some characters can't be saved in the chosen encoding and would be lost."** |
| `slides_to_pdf_flatten` | `PPTX/PPT/ODP → PDF` (presentations.md) | **"Animations, transitions and embedded media are flattened or dropped, and editing is no longer possible."** |
| `office_roundtrip_approx` | ODF↔MS office round-trip: `ODP → PPTX/PPT`, `PPTX → ODP` (presentations.md); also slide `→ PPTX/PPT` re-layout | **"Some effects and layout may shift when converting between PowerPoint and OpenDocument."** |
| `audio_lossy_target` | `→ MP3/AAC/M4A/OGG/OPUS` (audio.md) | **"Saved in a compressed audio format — some quality is reduced."** |
| `audio_transcode` | lossy source `→` lossy target (e.g. MP3→AAC) | **"Re-compressing already-compressed audio — quality drops a little more."** |
| `audio_lossy_origin` | lossy source `→` lossless target (e.g. MP3→FLAC) | **"This won't improve quality — the original is already compressed, so the result is just larger."** |
| `audio_bitdepth` | >16-bit source `→` default 16-bit WAV/AIFF | **"Saved at 16-bit — the source's extra audio precision is reduced."** |
| `audio_tags_dropped` | `→ AAC` (raw ADTS), partly WAV/AIFF | **"This format can't store song info, so title/artist tags are dropped."** |
| `video_reencode` | re-encode disposition (video.md / cross-cat) | **"Re-encoded to play widely — some video quality is reduced."** |
| `video_alpha_lost` | WEBM(alpha) `→ MP4/H.264` | **"Transparency isn't supported in this format and will be removed."** |
| `video_subs_dropped` | image/ASS subs `→ MP4` (subtitles policy) | **"Embedded subtitles couldn't be kept and were dropped."** |
| `video_to_gif` | `video → GIF` (cross-category, unconditional) | **"GIFs reduce colours, smoothness and remove sound — best for short clips."** |
| `audio_downmix` | surround forced to stereo by codec (rare) | **"Surround sound is mixed down to stereo for this format."** |

### 2.9.2 Note behaviour rules `[DECIDED]`

- **Predictable only.** A note appears **only** when the planned disposition is
  *known* to be lossy. For video, the note is keyed to the **planned remux-vs-
  re-encode decision** (§video.md Category-wide): a batch that will **remux** shows
  **no** note; one that will **re-encode** shows `video_reencode`. For a mixed batch
  where *any* item re-encodes, the note shows (honest worst-case) — per video.md.
- **One note, not a nag.** At most the relevant note(s) for the chosen target are
  shown together as calm inline lines; never a modal, never per-file, never a
  blocking acknowledgement (SSOT explicit).
- **Multiple kinds can co-apply** (e.g. animated WEBP→JPG = `image_animation_flatten`
  + `image_alpha_flatten` + `image_lossy_codec`). §5.7 renders the applicable set;
  *recommended:* de-duplicate to the most-specific 2–3 to avoid clutter.
- **Compatibility ≠ loss.** "This .opus may not open in older players" is **not** a
  §2.9 note — it is handled by the default-target tie-breaker (never defaulting to a
  modern format that may not open). §2.9 is strictly about **content faithfulness**.

---

## 2.10 Filenames & i18n (content + names) `[DECIDED]`

**Promise (SSOT *Never harm the original* / *Content fidelity*).** Real-world
filenames (any language, emoji, spaces, very long paths) are handled **without
mangling**; file *content* in any language (CJK, RTL), mixed encodings, and CSV
encoding/delimiters come through **intact, not mangled**.

### 2.10.1 Filenames `[DECIDED]`

- **Paths are OS-native opaque strings, not assumed-UTF-8.** Rust represents them as
  `PathBuf`/`OsString`. ConvertIA **never** lossily converts a path to `String`
  (no `to_string_lossy()` for any *operation* — only for *display* to the WebView,
  and even then via `to_string_lossy()` only at the very last step so a rare
  non-UTF-8 name is shown with the replacement char but still **operated on**
  losslessly via the original `OsString`).
  - **Windows** paths are UTF-16 (`OsStr` = WTF-8 internally) — emoji, CJK, combining
    marks survive round-trip.
  - **Unix** paths are arbitrary bytes — ConvertIA preserves the exact bytes.
- **The stem is preserved byte-for-byte** when forming the output name (§2.2) — only
  the extension changes and `(n)` may be appended. No transliteration, no ASCII-
  folding, no emoji stripping.
- **Unicode normalization caveat (macOS).** APFS/HFS+ historically normalise names
  toward **NFD**; Windows/Linux preserve as written (often **NFC**). ConvertIA does
  **not** re-normalise the stem itself (it preserves what the source had); the
  §2.3 identity check uses **inode/file-index**, not the name string, so an NFC-vs-
  NFD difference never causes a missed-identity or a duplicate. *Recommended:* do
  not attempt cross-OS name normalization in v1 — preserve verbatim and rely on
  identity-by-inode.
- **Long paths** are handled per §2.2.3: internally ConvertIA can use the Windows
  `\\?\` extended-length prefix for its **own** syscalls so it isn't itself blocked
  at 260, but a final *user-facing* path beyond the OS limit **fails clearly**
  (`PathTooLong`) — truncation is never the escape hatch.

### 2.10.2 Content fidelity `[DECIDED — delegated to engines + verified by corpus]`

§2.10 owns the *invariant*; the *per-engine mechanism* is in 04-formats and the
*reliability proof* is the SSOT corpus (§6.5). The invariant:

- **Text encoding is detected, never assumed from the extension** (documents.md /
  audio-tags policy): BOM → declared charset (`<meta>` / RTF code page / XML decl)
  → heuristic (UTF-8 → Windows-1252/Latin-1 → broader). Output text defaults to
  **UTF-8** (no BOM unless the target demands). CJK and **RTL** (Arabic/Hebrew)
  scripts pass through every engine path intact (this is a §6.5 corpus gate, not
  just an aspiration).
- **CSV** encoding + delimiter (`,` / `;` / `\t`) are detected and preserved per
  spreadsheets.md — never silently re-delimited or re-encoded.
- **Audio/video tags** in any script are preserved through the tag models that
  support UTF-8 (audio.md tag policy). Where a target can't store tags, that is the
  `audio_tags_dropped` §2.9 note — an honest, disclosed loss, not silent mangling.
- **Mixed/invalid byte sequences** → **fail clearly** (`Corrupt`/`EngineError`,
  §2.8) rather than emit mojibake (documents.md edge case) — "mangled" output is
  never an acceptable result.

---

## 2.11 Privacy & offline invariants `[DECIDED]`

**Promise (SSOT *Local, private & offline*).** Conversions happen on the user's
machine; user files are **never uploaded**; no accounts, no telemetry; **fully
self-contained, works completely offline**; **zero network access** for conversions;
no update check / phone-home; the only network is **user-initiated** (e.g. opening
the project page). The cloud-sync caveat is disclosed (ConvertIA can't control a
user's own OneDrive/iCloud/Dropbox).

### 2.11.1 The structural offline guarantee `[DECIDED]`

Offline is enforced **structurally**, not by policy, on two complementary halves:

- **WebView half (owned by §0.10).** The Tauri v2 **CSP** forbids all remote
  origins (`default-src 'self'`; `connect-src 'self' ipc:` only; no `http(s):`
  origins), and the **capabilities/permissions allowlist** grants the WebView no
  HTTP/fetch capability. §2.11 *requires* this; §0.10 *implements* it. Result: the
  UI **cannot** make a network request even if a dependency tried to.
- **Engine/core half (this section + §3.3).** **Every engine is bundled** (§3.3 —
  decided "bundle everything"), so no engine is fetched at runtime. Engines run as
  subprocesses inside the §2.12 isolation wrapper with **no network capability
  needed or granted**; the wrapper's sandbox profile (§2.12) can additionally
  **deny network syscalls** to the decoder processes as defence-in-depth.
  ConvertIA's Rust core makes **no outbound network calls** of any kind for a
  conversion — there is no HTTP client in the conversion path. Specific engine
  behaviours that *could* reach out are pinned off in 04: pandoc/LibreOffice/HTML
  rendering **do not fetch remote images/CSS** (documents.md: remote URLs become
  broken references, never fetched); SVG/`<image href>` is not fetched
  (images.md); these are content-fidelity *and* offline guarantees.

### 2.11.2 No telemetry / accounts / update phone-home `[DECIDED]`

- **No accounts, no telemetry** — there is no analytics SDK, no crash reporter that
  transmits, no usage beacon. The local log (§7.5) is local-only and never sent.
- **No auto-updater / no phone-home** — the Tauri updater is **explicitly disabled/
  absent** (§7.6 owns the concrete config item). ConvertIA does **not** check for
  updates. The "new version available" path is **user-initiated only** (the About
  screen §5.9 links to the canonical GitHub Releases page; clicking it is the *only*
  network activity, routed through §7.7 shell-out to the OS browser — ConvertIA
  itself still makes no request).

### 2.11.3 The cloud-sync caveat (disclosed, not enforced) `[DECIDED]`

ConvertIA writes outputs **beside the source by default** (§2.7). If the source sits
in a cloud-synced folder (OneDrive/iCloud/Dropbox/corporate share), the **user's own
sync client** may upload the originals and the results. ConvertIA **neither causes,
prevents, nor detects** this (SSOT). This is **disclosed in the About screen** (§5.9
chrome) — §2.11 owns the *invariant statement* ("private = nothing leaves the
machine **as a result of what ConvertIA does**"); §5.9 owns the *wording shown*.

### 2.11.4 Observability of "no network" (a v1 DoD gate) `[DECIDED]`

The SSOT v1-DoD requires the offline guarantee be **observably true**. §6.x (test
strategy) owns the *test*; §2.11 fixes *what is asserted*: with the machine offline
(or watched by a packet monitor / OS firewall logger), a **full conversion of every
category produces zero outbound packets**, and the app launches and converts
identically with networking disabled. This is a release gate, not a runtime check.

---

## 2.12 Security / decoder isolation `[DECIDED — single owner here]`

**Promise (SSOT *Security posture*).** ConvertIA opens **arbitrary, possibly
malicious** files through third-party decoders. Decoding untrusted input is
**isolated/contained** so a decoder crash or hang **fails that one item clearly**
(per *Fail clearly*) **without wedging the app or compromising the no-harm
guarantee**.

> **Ownership.** §2.12 is the **single owner of the per-platform decoder-isolation
> mechanism**. §0.3 (process model) and §1.7 (invocation lifecycle) **route
> through** it; §3.5 builds the engine arguments **inside** the wrapper it defines.
> It pairs with §0.10 (the WebView/CSP half of security) and is one entry in the
> §0.11 threat-surface map (threat class: *untrusted decoder input*).

### 2.12.1 The isolation primitive: process boundary (already in the architecture) `[DECIDED]`

Every engine already runs as a **separate OS subprocess** (§0.3 process model; §3.6
copyleft isolation makes this mandatory anyway). That process boundary **is** the
first and primary isolation layer: a decoder that segfaults, aborts, or corrupts its
own heap takes down **only its own process**, never the Tauri core or the WebView.
This satisfies the SSOT minimum directly:

- **Crash containment:** subprocess death → §1.7 reaps it → maps to `EngineCrash`
  (§2.8) → that one item fails, batch continues. The Rust core's worker that was
  waiting on the child observes the abnormal exit; nothing in the core is unwound by
  the child's crash (separate address space).
- **No-harm preserved across a decoder crash:** the decoder only ever writes to its
  private `tmp` (§2.1); `final` was never created. A mid-decode crash leaves only a
  discardable `*.part` (§2.6). The crash cannot produce a truncated `final`.

### 2.12.2 Hang containment `[DECIDED]`

A decoder that **hangs** (infinite loop on a crafted file, a decompression stall) is
bounded by the §1.7 **timeout/kill**: after the per-job timeout (parameters owned by
§0.9, mechanism by §1.7), the subprocess is killed via §1.7's process-group kill
(Unix `kill(-pgid, SIGKILL)`; Windows Job Object `TerminateJobObject` — Windows has
no SIGTERM, §1.7) → `EngineHang` (§2.8). The app stays responsive throughout (the
core is async; the hung child is just a pending future that gets cancelled).

### 2.12.3 Hardening the subprocess (defence-in-depth) `[OPEN — recommended tiers]`

Beyond the process boundary, ConvertIA **should** drop the decoder's privileges so a
*compromised* (not merely crashing) decoder can do minimal damage. The mechanism is
**per-OS** and is the genuine `[OPEN]` here (it has real cost/portability
trade-offs). Recommended, in priority order:

- **All platforms (cheap, v1):** spawn each engine with **(a)** a working directory
  set to the **per-run scratch dir** (§2.6) so relative paths can't wander; **(b)**
  a **minimal environment** (cleared env except what the engine needs — no inherited
  secrets); **(c)** the §2.12.1 process boundary; **(d)** the §1.7 timeout. The
  engine is handed **only** the exact input path and the `tmp` output path (§3.5),
  not a directory it can scan.
- **Linux (recommended v1 if feasible):** wrap the spawn in a **seccomp-bpf** filter
  (e.g. via the `seccompiler`/`extrasafe` crate) denying network + exec + unexpected
  syscalls, and/or **Landlock** (kernel ≥ 5.13, `landlock` crate) restricting the
  decoder's filesystem to `{input file (ro), tmp dir (rw)}`. Network is denied so
  the offline guarantee (§2.11) is enforced even on a hostile decoder.
- **macOS (recommended v1 if feasible):** run the engine under a **`sandbox-exec`
  profile** / Seatbelt SBPL restricting it to read the input + write the scratch dir,
  deny network and process-exec. (Apple deprecates `sandbox-exec` as a CLI but the
  underlying `sandbox_init` profile mechanism remains; portable-build constraints
  apply.)
- **Windows (recommended v1 if feasible):** spawn in a **restricted token / App
  Container or Job Object** with **`JOB_OBJECT_LIMIT`** flags (kill-on-job-close so
  no orphan survives, memory cap), a **low-integrity** token, and network disabled
  via the Job/firewall. The Job Object is also what §1.7 uses for group-kill, so this
  is shared infrastructure.

> `[OPEN]` (owner §2.12): **how deep the v1 sandbox tier goes per OS.** The
> process-boundary + timeout + minimal-env + scratch-cwd tier is **non-negotiable
> v1** (it is what the SSOT *requires*). The seccomp/Landlock/Seatbelt/Job-Object
> *privilege-drop* tier is a **strong recommendation** but carries portability risk
> (kernel/OS-version variance, the "portable, no-installation" constraint must not
> need elevated rights to *run* the sandbox). **Recommendation: ship the cheap tier
> in v1 on all three OSes, and the privilege-drop tier where it works without
> requiring install-time privileges or breaking the portable build — degrading
> gracefully to the cheap tier if a given machine can't enable it.** Flagged because
> the exact per-OS depth is a real engineering decision feeding §0.11 and §6.

### 2.12.4 Where detection runs relative to the boundary `[DECIDED]`

Detection (§1.2) is the **first code touching untrusted bytes**. ConvertIA's
detection is **header/magic-byte sniffing only** (a bounded read of the first N
bytes + light structure checks), implemented in **safe Rust** with **no full
decode** — so it is acceptable to run **in-core** (it doesn't invoke a third-party
decoder). The moment a full decode is needed (the actual conversion), that runs in
the isolated subprocess. §1.2 states this; §2.12 confirms the boundary: *no
third-party decoder library is linked into or run inside the Rust core* — they are
all subprocesses. (This also reinforces §3.6: copyleft engines are aggregated as
separate binaries, never linked into the MIT core.)

---

## 2.13 App-level fault model (vs per-item) & the "no stack traces" contract `[DECIDED]`

**Promise (SSOT *Fail clearly*).** No stack traces; an unexpected internal error is
shown to a non-technical user calmly. This section defines the **fault classes** and
how each surfaces without a trace.

### 2.13.1 Three fault classes `[DECIDED]`

| Class | Examples | Scope of impact | Where surfaced |
|-------|----------|-----------------|----------------|
| **Item-level** | corrupt file, engine crash on one input, too-big, out-of-disk | **one item** fails; batch continues | §2.8 catalog → §1.12 summary |
| **Run-level** | scratch volume vanished mid-run, the *whole batch* hits out-of-disk up front, every item fails | the **run** can't proceed sensibly | §2.8 batch summary ("None could be converted…") |
| **App-level** | Rust core **panic**, WebView fails to load, an engine binary **missing/corrupt at startup**, **damaged bundle**, **no disk at all**, missing/old WebView runtime | the **app** can't function | §2.13.3 calm app-level screen + §7.2 startup faults |

Item-level is §2.8's domain. Run-level reuses §2.8's batch strings. App-level is
this section.

### 2.13.2 The worker-thread panic boundary `[DECIDED]`

ConvertIA's conversion workers (the async tasks / thread pool, §0.9) wrap each item's
processing in a **panic boundary** so a bug-induced panic in *our* orchestration
code (not the engine — that's a subprocess, §2.12) **isolates to one item** instead
of poisoning the pool:

- Each item's core-side work runs inside **`std::panic::catch_unwind`** (with the
  closure made `AssertUnwindSafe` as needed). A caught panic is converted to
  `ConversionError::InternalError` (§2.8) for that item — **the batch continues**.
- The panic payload (message + location) is **logged locally only** (§7.5, if
  enabled, redacted); the **user sees only** the calm `InternalError` string — **no
  stack trace** (SSOT). We **do not** `resume_unwind` on the worker (that would kill
  the pool); we recover at the item boundary, matching the thread-pool pattern
  (catch at the pool boundary, report to the client).
- `panic = "unwind"` (the default) is **required** in `Cargo.toml` for release so
  `catch_unwind` works; `panic = "abort"` is **not** used for the app binary
  (it would turn a recoverable per-item bug into a whole-app crash). Engines are
  separate processes, so their abort behaviour is irrelevant to this.

### 2.13.3 App-level fault presentation (no trace) `[DECIDED]`

When a fault is genuinely **app-level** (the core cannot continue, or a startup
precondition fails), ConvertIA shows a **single calm screen**, never a crash dialog
with a trace:

- **Startup faults** (engine binary missing/corrupt, damaged bundle, missing/old
  WebView runtime, no writable scratch at all) are detected by the §7.2 startup
  sequence **before** the user can drop anything. They render a plain message —
  e.g. *"ConvertIA can't start because part of the app appears to be missing or
  damaged. Try downloading it again from the official releases page."* — owned by
  §7.2 (link to §5.9 About / canonical releases). §2.13 fixes that these are
  **app-level** and **trace-free**; §7.2 owns the exact sequence and the strings
  shown at the boundary.
- **Mid-run core panic that escapes the item boundary** (should be impossible, but
  defended): a top-level handler shows *"Something went wrong and ConvertIA needs to
  recover. Your original files are safe and untouched."* (true by §2.1/§2.12 — no
  `final` was ever clobbered) and returns to the idle state; the detail is logged
  locally only.
- **WebView/backend disconnect** (the UI loses the IPC channel, §5.8) shows a calm
  "reconnecting / restart" affordance — §5.8 owns the UI handling; §2.13 owns that
  it is a no-trace app-level class.

### 2.13.4 Engine `stderr` capture-and-classify feeds §2.8 `[DECIDED]`

Each engine subprocess's **`stderr` is captured** (never shown raw to the user). §3.5
owns the per-engine stderr quirks; §1.7 owns the exit-code mapping; §2.13 fixes the
**rule**: captured stderr/exit are **classified** into a §2.8 kind
(`EngineError`/`EngineCrash`/`PasswordProtected`/`Corrupt`/…). Unclassifiable output
maps to the generic `EngineError` calm string — the raw text goes only to the local
log (§7.5). **The user never sees engine stderr.**

---

## 2.14 Temp / scratch space & cross-volume atomic strategy `[DECIDED — single owner here]`

**Promise (derived from SSOT *Never harm the original*).** Atomic rename (§2.1)
requires the temp + final to be on the **same filesystem** (the OS `rename`/
`MoveFileEx` is intra-volume; cross-device → **`EXDEV`** on Unix / failure on
Windows). But beside-source default + per-location divert (§2.7) can put **source,
scratch and final on three different volumes** (USB source → Downloads divert on the
system disk). This section is the **single owner** of where scratch lives, how the
final move stays atomic, and the cross-volume fallback.

> **Ownership.** §2.1 / §2.6 / §1.10 / §3.5 / §7.2 **reference** this instead of each
> implying its own temp model. §2.14 is the one place the volume question is answered.

### 2.14.1 Same-volume rule: scratch goes next to the *final*, not next to the *source* `[DECIDED]`

The atomic-publish (§2.1.2) is a `rename(tmp → final)`, which is only atomic
**within one volume**. Therefore the **invariant**:

> **`tmp` is always created on the same volume as `final`** (the *destination*), not
> necessarily the same volume as the source.

Concretely, `core::run` picks the per-job scratch path **inside the destination
directory's volume**. The simplest correct choice: a **hidden per-run scratch
subdir inside (or adjacent to) the destination directory itself** —
`…/<dest_dir>/.convertia-run-<RunId>/<job>.part` — guaranteeing same-volume by
construction. This is what makes the §2.1 rename a true atomic publish in the common
beside-source case (dest dir = source dir = one volume) **and** in the divert case
(dest dir = Downloads = system volume; scratch also on the system volume).

- *Recommended scratch placement:* **inside the destination directory** (a dotted
  run-dir), removed by §2.6 on run end. This avoids any cross-volume move for the
  *publish*. If the destination directory itself is not writable, §2.7 has **already
  diverted** the destination to a writable one — so by the time §2.14 places scratch,
  the destination is known-writable (§2.7.2 probe).
- *Alternative considered & rejected for the publish:* a single global app scratch
  dir (e.g. under the OS temp) for *all* runs. Rejected as the *publish* temp because
  it is frequently on a **different volume** than a beside-source destination,
  forcing the cross-volume fallback (2.14.3) on the **common** path. The global temp
  is fine for **transient engine working files** that are *not* the publish artifact
  (see 2.14.2).

### 2.14.2 Two kinds of scratch `[DECIDED]`

ConvertIA distinguishes:

1. **The publish temp (`*.part`)** — the file that becomes `final` via atomic
   rename. **Must** be on `final`'s volume (2.14.1).
2. **Engine working files** — anything an engine writes transiently that is *not*
   the final artifact (e.g. a LibreOffice user-profile dir per run, FFmpeg's
   internal temp, the per-run isolated profile §documents.md). These **need not** be
   on the destination volume and live under the **per-run scratch root** chosen via
   Tauri v2 `PathResolver` (`app_local_data_dir()`/`temp_dir()`), keyed by `RunId`
   (§2.6). They are cleaned with the run.

The LibreOffice per-run isolated user profile (documents.md *Edge cases*; §0.9 notes
LibreOffice headless is **not** safely parallel under one profile) is a **kind-2**
working file: it lives in the per-run scratch root, one profile per run, so serialized
LibreOffice invocations don't collide.

### 2.14.3 Cross-volume fallback (only when same-volume can't be guaranteed) `[DECIDED]`

In the rare case where the publish temp truly cannot be co-located with `final` on
one volume (e.g. a destination dir that is writable but on a filesystem where
creating a sibling scratch dir is disallowed, or a quirky network mount), the
**fallback preserves atomicity *within the destination volume*** by doing the
move-equivalent **inside** that volume:

1. Write `tmp` in the **best same-volume location obtainable** for `final` (the
   destination dir; if a sibling dotdir can't be made, the destination dir's own
   parent on the same volume).
2. If, despite this, the only available scratch is on **another** volume, perform a
   **copy + fsync + atomic-rename-within-destination-volume**:
   - copy the cross-volume temp into a **new** temp **on `final`'s volume**,
   - `sync_all()` it (durable),
   - then `rename` that same-volume temp → `final` (atomic, intra-volume),
   - `fsync` the destination directory (Unix) for durability.
   This is exactly the documented `EXDEV` remedy (the tempfile-crate guidance:
   *cannot persist across filesystems → copy into the destination volume, then
   rename*). The cross-volume step is a **copy**, never a cross-volume `rename`
   (which would fail `EXDEV`); the **only** rename is intra-volume and atomic.
3. The extra copy is removed by §2.6. The user-visible result is identical: `final`
   appears atomically or not at all; a crash leaves only discardable temps.

`fs_guard::atomic_publish(tmp, final)` encapsulates all of this: it tries the
direct intra-volume rename first, and only on `EXDEV` (Unix) / cross-device failure
(Windows) falls back to copy-into-dest-volume-then-rename. Callers (§2.1) never see
the distinction.

### 2.14.4 Space accounting ties to §1.10 `[DECIDED]`

The scratch model means a conversion transiently needs **destination-volume free
space ≈ output size** (publish temp) **plus** any kind-2 working space. §1.10 (resource
pre-flight, `[OPEN]` budgets) owns the up-front estimate and the "doomed for disk"
fast-fail; §2.14 **supplies** the model it estimates against: *publish temp lands on
the destination volume*, so §1.10's free-space check must target the **destination**
volume, not the source volume. The to-GIF guardrail (cross-category.md) and video
re-encode estimates feed the same §1.10 check on the destination volume.

---

## Cross-section reference index (what 02 hands to / takes from)

| 02 mechanism | Owns | References (does not restate) |
|--------------|------|-------------------------------|
| Atomic write (§2.1) | the write sequence, exclusive-create, durability | scratch volume → §2.14; engine spawn → §1.7/§2.12; output plan → §1.8 |
| Naming (§2.2) | name shape, numbering loop, path-limit fail | target extension → 04-formats; re-run-vs-number split → §2.5 |
| Link safety (§2.3) | identity model, write-target check, dedup | divert target → §2.7; frozen-set build → §2.4/§1.1 |
| Frozen set (§2.4) | snapshot semantics, no-self-feed | folder recursion → §1.1; instance hand-off → §7.1/§7.8 |
| Re-run (§2.5) | equivalence key + best-effort prompt/fallback | prompt UI → §5.2; settings values → §1.6/04; persistence → §7.4 |
| Cleanup (§2.6) | temp ownership, sweep, residue honesty | RunId/liveness → §7.1; scratch root → §2.14 |
| Destination (§2.7) | beside/chosen/divert rules, guarantees-on-divert | OutputPlan compute → §1.8; "will save to" UI → §5.2; open-folder → §7.7 |
| Error taxonomy (§2.8) | **kinds + message catalog** | exit-map → §1.7; stderr quirks → §3.5; batch-continue → §1.9; surfacing → §5.7 |
| Lossy (§2.9) | **lossy-note string catalog** | which pairs are lossy → 04 flags; passive surfacing → §5.7 |
| i18n (§2.10) | filename/content invariants | per-engine encoding → 04; corpus proof → §6.5 |
| Privacy/offline (§2.11) | the invariants + cloud-sync caveat statement | CSP/allowlist → §0.10; bundling → §3.3; updater-off → §7.6; cloud-sync wording → §5.9 |
| Decoder isolation (§2.12) | **per-OS isolation mechanism** | spawn lifecycle → §1.7; args → §3.5; CSP half → §0.10; threat map → §0.11 |
| App fault (§2.13) | fault classes, panic boundary, no-trace contract | startup faults → §7.2; UI disconnect → §5.8; concurrency → §0.9 |
| Temp/cross-volume (§2.14) | **scratch volume policy + EXDEV fallback** | RunId/cleanup → §2.6; budgets → §1.10; PathResolver → §0.8/§7 |
