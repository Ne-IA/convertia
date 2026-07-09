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

All mechanisms below are implemented **once**, in the **guarantees-fs layer** owned by
§0.7 (not duplicated per engine or per format). **Naming, fixed `[DECIDED]`:** the
*conceptual layer* is "guarantees-fs"; its *canonical Rust module path* is
**`crate::fs_guard`** and its *physical directory* is `src-tauri/src/fs_guard/` (§0.7) —
one module, three context-appropriate labels, **no** `fs_guarantees` module name floats
anymore. (The path is `crate::fs_guard`, **not** `core::fs_guard` — in a Rust binary
crate `core` resolves to the no_std standard-library crate, so an application module may
not be named `core`; the app crate's modules are addressed `crate::…`.) Logical home:

- `crate::fs_guard` — atomic write, no-clobber, resolved-identity, path-limit checks
  (§2.1 / §2.2 / §2.3 / §2.14).
- `crate::run` — per-run/instance scratch ownership and cleanup (§2.6), keyed on the
  `RunId`/`InstanceId` defined by §7.1.
- `crate::outcome` — the error taxonomy + message catalog (§2.8) and lossy catalog
  (§2.9), the **single source of every conversion-outcome string**.
- `crate::isolation` — the decoder-isolation wrapper (§2.12) every engine spawn
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

1. **Pick the publish-temp path** `tmp` on the **same volume as `final`** (§2.14
   owns *which* volume and the cross-volume fallback). Per §2.14.1 the publish temp
   is a uniquely-named **sibling file in the destination directory** (not the central
   scratch root, which is frequently on a different volume), e.g.
   `…/<dest_dir>/.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`. Ownership is
   encoded in the `InstanceId`+`RunId` so cleanup (§2.6) can identify it **and resolve its
   exact owning lock cross-instance** (§2.6.3 / §2.14.1).
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
5. **Publish `tmp → final` with the no-placeholder exclusive-rename** (2.1.2): a
   primitive that creates `final` **only if it does not exist** — Linux
   `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)` / common
   `link`+`unlink` fallback, Windows `MoveFileExW` **without**
   `MOVEFILE_REPLACE_EXISTING` (create-only, fails-if-exists). No 0-byte placeholder
   is ever created at the final name (so no truncated/empty `final`, §2.1.3). On a name
   collision the loop advances to the next §2.2 variant.
6. **Durability of the publish:** on Unix, after the rename **fsync the containing
   directory** (open the parent dir, `fsync` its fd) so the new dentry survives a
   crash — per the LWN/evanjones durability findings (rename is atomic but not
   durable without the directory fsync). **On the `link`+`unlink` fallback path** (either
   Unix OS lacking the single-call no-replace primitive) the **same directory fsync applies**:
   after the `link(tmp, final)` succeeds, **fsync `final`'s parent directory** so the new
   `final` dentry is durable (the file *bytes* are already durable — `link` shares the inode
   `sync_all`'d in step 3, so no second byte-fsync is needed; only the new dentry needs the
   dir-fsync). On Windows the directory-fsync step is a
   no-op (NTFS metadata journaling covers the dentry). Durability sources on Windows
   are split: the **file *bytes*** are made durable by `sync_all` on the temp handle
   before the move (as on Unix); the **`MOVEFILE_WRITE_THROUGH`** flag on the
   create-only `MoveFileExW` flushes the *move/metadata* through — its documented
   effect is for the **cross-volume copy-and-delete** form; for a same-volume move the
   dentry's durability rests on NTFS journaling. This is a strong best-effort, not a
   byte-for-byte equivalent of the Unix dir-fsync, so we do not claim them identical.
   **Atomicity (the no-third-state §2.1.3 invariant) comes SOLELY from
   `MoveFileExW`-*without*-`MOVEFILE_REPLACE_EXISTING` (create-only)** — `WRITE_THROUGH`
   is only a best-effort metadata flush and the crash-safety invariant does **not**
   depend on it. (No replacing path exists; `ReplaceFileW`/`REPLACEFILE_WRITE_THROUGH`
   have no caller — §2.1.2 / §2.5.2.)
7. On engine failure / cancel / any error in steps 3–6: **`tmp` is removed**
   (§2.6); `final` was never created → nothing to undo. Cleanup failure is itself
   handled (§2.6: never reported as clean success).

### 2.1.2 Exclusive create + atomic publish — the OS-primitive split `[DECIDED]`

The **no-clobber** part and the **atomic-publish** part are the **same single
no-placeholder exclusive-rename**: no single cross-platform call exclusive-creates
*and* fills from a temp atomically, so the engine writes a private `tmp` and we publish
`tmp → final` with a primitive that **creates `final` only if it does not exist**. The
publish IS the no-clobber check — there is **no separate `create_new` placeholder
reserve at `final`** (a reserve-then-rename would reintroduce the forbidden third
state; see the "Why no-placeholder" callout and the **rejected** option (b) below).

The publish, given a candidate `final` name, uses a **no-placeholder publish**: the
final name is created **exactly once, by a rename that fails-if-exists**, so no empty
placeholder is ever published and no third state (a 0-byte `final` we own but the
engine never filled) can exist. On a name collision the loop advances to the next §2.2
variant and retries — a bounded retry loop (cap ~10 000 variants, then path-limit /
too-many-collisions failure §2.8). This is what makes the guarantee **absolute against
concurrent writers** (a second instance, a concurrent conversion, a file that appeared
meanwhile): the *kernel* enforces "new or fail" at publish, not a prior `exists()`
check. `[DECIDED]`

> **"Exclusive create" everywhere means this publish, not a `create_new`
> placeholder.** Where §2.1.1 step 4, §2.2.2, §2.3.3 and §2.6.2 say "exclusive
> create", they mean **this no-placeholder exclusive-rename publish** (Linux
> `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)` / common
> `link`+`unlink` fallback; Windows `MoveFileExW`-without-`REPLACE_EXISTING`
> / the §2.3.3 dir-handle-relative `NtSetInformationFile` form) — **never** an
> `OpenOptions::create_new(true).open(final)` that would leave a 0-byte `final`.

- **(a) No-placeholder exclusive-rename (chosen).** The engine writes to a private
  `tmp`; we then publish `tmp → final` with a primitive that **creates the name
  atomically only if it does not exist** — no prior `create_new` placeholder:
  - **Unix — named per platform `[DECIDED]`:** the single-call exclusive
    create-as-publish primitive **differs between Linux and macOS** (both create-only /
    no-replace; both fall back to `link`+`unlink`):
    - **Linux:** `renameat2(..., RENAME_NOREPLACE)` (Linux ≥ 3.15) — fails `EEXIST` if
      `final` exists.
    - **macOS:** `renameatx_np(..., RENAME_EXCL)` (or `renamex_np` for the non-`at`
      form) — the macOS equivalent of `RENAME_NOREPLACE`; it returns `EEXIST` if the
      destination exists on filesystems that advertise `VOL_CAP_INT_RENAME_EXCL`
      (`getattrlist(2)`). **macOS has NO `renameat2`/`RENAME_NOREPLACE`** — using the
      Linux spelling on macOS would not compile / would silently always fall to
      `link`+`unlink`, so the platform-correct call must be `renameatx_np(RENAME_EXCL)`.
    - **Common fallback (both OSes):** `link(tmp, final)` then `unlink(tmp)` — the
      portable POSIX form, used when the single-call no-replace primitive is unavailable
      or the filesystem does not support the flag (Linux `EINVAL`; macOS filesystems
      without `VOL_CAP_INT_RENAME_EXCL`). It fails `EEXIST` if `final` exists.
    - **Third fallback — neither no-replace rename NOR hardlinks (FAT/exFAT-class)
      `[DECIDED]`.** On a destination filesystem that supports **neither** the single-call
      no-replace primitive **nor** hardlinks — the canonical case being **FAT32/exFAT** (the
      portable-USB destination of §2.14.2) — the `link`+`unlink` fallback itself **fails**
      (`link()` → `EPERM`/`ENOTSUP`, since FAT/exFAT have no hardlink support). There is
      therefore **no mechanised create-only / atomic no-clobber publish primitive on Unix**
      for such a destination. ConvertIA does **not** silently weaken the no-clobber/atomic
      guarantee there: such a destination is **detected up front at §2.7.2 `location_status`
      time and treated as a per-location DIVERT trigger** ("cannot guarantee atomic
      no-clobber here" → divert to the hardlink-capable system-disk target, §2.7.3), so the
      full §2.1 publish chain runs on a volume that supports it. (This is **Unix-only**:
      Windows' `MoveFileExW`-without-`MOVEFILE_REPLACE_EXISTING` is a true create-only move
      on FAT/exFAT too, so a Windows FAT/exFAT destination keeps the guarantee in place and
      is **not** diverted, §2.7.2.)

    Each of the first two gives the no-clobber guarantee **and** the atomic publish in one
    step, with no placeholder. On `EEXIST` → re-pick the next §2.2 variant.
    - **Link-form success-window residual `[DECIDED]`.** Unlike the single-call primitive
      (Linux `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)`, which
      consumes `tmp` atomically), the `link`+`unlink` fallback has a brief window
      **after `link` succeeds but before `unlink(tmp)`** where **both** `final` and
      the `tmp` `*.part` exist — a residual `.part` on the *success* path (the
      single-call path has none). This is benign (`final` is already complete and
      durable) but means §2.6.2's "item success → nothing to remove" is true only on
      the single-call path; on the link path the `unlink(tmp)` is the removal, and if
      it fails the leftover `*.part` is reclaimed by the §2.6.4 sweep (annotated as
      a residue, not an item failure). See the §2.1.3 link-form sub-state.
  - **Windows `[DECIDED]`:** the first-time (no-clobber) publish is a
    **create-only move with no placeholder**: **`MoveFileExW(tmp, final,
    MOVEFILE_WRITE_THROUGH)`** — i.e. **WITHOUT** `MOVEFILE_REPLACE_EXISTING`. With
    `REPLACE_EXISTING` omitted, `MoveFileExW` **fails (`ERROR_ALREADY_EXISTS` /
    `ERROR_FILE_EXISTS`) if `final` exists**, giving the no-clobber guarantee **and**
    the publish in one step **with no 0-byte placeholder ever created at the final
    name** — the exact create-only shape of the Unix no-replace primitive (Linux
    `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)` / `link`+`unlink`),
    so the §2.1.3 two-state invariant holds by construction on Windows too.
    On the exists-error → re-pick the next §2.2 variant. **(Parent-swap nuance:** the
    path-string `MoveFileExW` re-resolves `final` by path at publish time, so to *also*
    close the §2.3.3 parent-directory-swap race the publish is issued in its
    **dir-handle-relative form — `NtSetInformationFile(…, FileRenameInformationEx)` with a
    `FILE_RENAME_INFORMATION_EX` whose `RootDirectory` is the verified parent dir HANDLE
    and whose `Flags` bitfield OMITS `FILE_RENAME_REPLACE_IF_EXISTS` (the Ex class's
    no-replace; NOT the boolean `ReplaceIfExists` of the non-Ex struct) →
    `STATUS_OBJECT_NAME_COLLISION` on collision** — see §2.3.3. Same create-only,
    no-placeholder semantics; rooted at a handle, not a re-parsed path.)
    - The earlier `create_new`-reserve-then-`ReplaceFileW` ordering is **rejected**
      precisely because it first creates a 0-byte file at the **final** path
      (`ReplaceFileW` requires the target to exist), admitting the forbidden third
      state if a crash lands between the reserve and the replace.
    - **There is NO replacing-publish path in v1.** `ReplaceFileW` (the NTFS
      atomic-*replace*, which requires the target to exist) has **no caller**: the §2.5
      re-run **FreshCopy** decision does **not** replace an existing file — it falls
      through to **ordinary §2.2 next-free-variant numbering** (a create-only publish at
      the next non-existing name, §2.5.2). The absolute no-clobber rule (§2.1) means a
      same-named file is treated as an unrelated collision and is **never** overwritten,
      so a genuinely-replacing primitive would violate the guarantee. Windows publish is
      therefore **always** the create-only `MoveFileExW`-without-`REPLACE_EXISTING`;
      `MOVEFILE_REPLACE_EXISTING` and `ReplaceFileW` are **never** used.
    - **AV interference:** `MoveFileExW` can return `ERROR_ACCESS_DENIED` when antivirus
      holds a transient open handle on `tmp`; a **bounded retry** (short backoff, small
      cap, then `WriteFailed` §2.8) is applied.
- **(b) Write-into-the-reserved-handle.** Stream the engine output through an open
  exclusive handle directly (no temp + rename). **Rejected for the engine path**:
  engines are *separate processes* writing their own file (§3.5) — they cannot
  share our Rust file handle, and they may write non-atomically. (b) is only viable
  for in-core writes, which ConvertIA has none of (every output is engine-produced).

> **Why no-placeholder.** A reserve-then-rename design (create a 0-byte placeholder,
> then rename `tmp` over it) reintroduces a forbidden **third state**: a crash
> between the placeholder create and the rename leaves a 0-byte `final` the engine
> never wrote — exactly the "truncated/empty final masquerading as finished" §2.1.3
> forbids. The no-placeholder publish (Linux `renameat2(RENAME_NOREPLACE)` / macOS
> `renameatx_np(RENAME_EXCL)` / common `link`+`unlink` fallback;
> Windows `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`) never creates an empty
> name, so the §2.1.3 two-state invariant holds by construction. `fs_guard::
> atomic_publish(tmp, final)` encapsulates the per-OS primitive choice; callers (§2.1)
> never see it.
>
> `[DEFER: primitive-confirmation spike, not a design question]` (owner §2.1):
> confirm `renameat2(RENAME_NOREPLACE)` (Linux) / `renameatx_np(RENAME_EXCL)` (macOS)
> availability across the §0.3.1 floor, with the `link`+`unlink` fallback **and the
> third fallback (FAT/exFAT-class: neither no-replace rename nor hardlinks → §2.7.2
> divert)**. The spike also confirms the §2.7.2 FAT/exFAT-class detection (filesystem-type
> query and/or one-shot `EINVAL`-then-`EPERM` capability probe). **The fallback is chosen at
> runtime PER DESTINATION, not statically `[DECIDED]`:**
> `renameat2(RENAME_NOREPLACE)` returns **`EINVAL` on Linux filesystems that don't
> support the flag**, and `renameatx_np(RENAME_EXCL)` is **only honoured on macOS
> filesystems that advertise `VOL_CAP_INT_RENAME_EXCL`** (some USB/network/FUSE mounts
> differ from the boot volume on the same machine), so `atomic_publish` tries the
> single-call primitive and, on the unsupported error, **falls back to `link`+`unlink`
> for that destination** (not a build-time kernel-version switch — the same kernel can
> have both). On **NFS**,
> where a rename result can be ambiguous, treat an ambiguous outcome as
> **name-may-be-taken** and re-pick the next §2.2 variant (never assume success). The
> Windows primitive is **fixed**: the
> publish is **always** `MoveFileExW` *without* `MOVEFILE_REPLACE_EXISTING` (a clean
> create-only move, no placeholder). **There is no replacing path** — the §2.5 re-run
> FreshCopy uses ordinary §2.2 create-only numbering, never replacement, so
> `ReplaceFileW`/`MOVEFILE_REPLACE_EXISTING` have no caller (§2.5.2). The
> *guarantee and the primitive choice are both fixed*; only the Linux availability
> check remains, and it has a guaranteed fallback.

### 2.1.3 Crash / power-loss invariant `[DECIDED]`

After any ungraceful end, the on-disk state is exactly one of:

- **`final` exists and is complete** — the rename (step 5) committed; `sync_all`
  (step 3) + dir-fsync (step 6) guarantee its bytes are durable. *Success.*
- **`final` does not exist, a `*.part` temp may remain** — the rename had not yet
  committed. The temp is a **discardable run-owned artifact**, cleaned on next run
  (§2.6). *No partial output masquerading as finished.*
- **(`link`+`unlink` fallback only) `final` exists AND a `*.part` temp also remains**
  — the crash landed in the success window *after* `link(tmp, final)` committed but
  *before* `unlink(tmp)`. `final` is complete and durable (Success); the leftover
  `*.part` is a discardable run-owned artifact reclaimed by the §2.6.4 sweep. The
  residual-free single-call path (no leftover) exists on **BOTH** Linux
  (`renameat2(RENAME_NOREPLACE)`) and macOS (`renameatx_np(RENAME_EXCL)`) as well as
  Windows (`MoveFileExW`), so this sub-state is the **`link`+`unlink`-fallback case on
  EITHER Unix OS** (where the single-call no-replace primitive is unavailable / the
  filesystem does not support the flag) — **not** a macOS-always penalty. It is still
  **not** a truncated-final state.

There is **never** a third state (a truncated or 0-byte `final`) because (1) the
engine only ever writes to `tmp`, never to `final`, and (2) the publish is a
**no-placeholder** exclusive-rename (§2.1.2) — `final` springs into existence already
complete, in one atomic step, with **no intermediate empty placeholder** that a
crash could leave behind. (The earlier reserve-then-rename design, which created a
0-byte placeholder first, *would* have admitted that third state on a crash between
reserve and rename; it is rejected in §2.1.2 precisely for that reason.) This
satisfies the SSOT "holds even across an ungraceful end". Cross-volume nuance (when
`tmp` and `final` cannot be on the same volume) is §2.14 — there the only rename is
still intra-volume and exclusive, so the same two-state invariant holds.

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

### 2.2.2 Collision discovery is via §2.1's exclusive publish, not a pre-scan

`n` is **not** chosen by listing the directory and picking max+1 (that is itself a
TOCTOU race). Instead `output_name` produces candidates **lazily** and each
candidate is handed to **the canonical §2.3.3 dir-handle-relative no-placeholder
exclusive publish** — **the same single primitive every publish (incl. every
numbering-loop retry) uses**, rooted at the **verified parent-directory handle** so the
parent-swap TOCTOU is closed (§2.3.3): Unix `linkat` / Linux `renameat2(olddirfd, tmp,
dirfd, leaf, RENAME_NOREPLACE)` / macOS `renameatx_np(olddirfd, tmp, dirfd, leaf,
RENAME_EXCL)` → fails `EEXIST`; Windows
`NtSetInformationFile(tmpHandle, …, FileRenameInformationEx)` with `RootDirectory` = the
verified parent handle and the `FILE_RENAME_INFORMATION_EX` `Flags` bitfield **OMITTING**
`FILE_RENAME_REPLACE_IF_EXISTS` → fails `STATUS_OBJECT_NAME_COLLISION`. (The bare
path-string `MoveFileExW(tmp, final)` WITHOUT `MOVEFILE_REPLACE_EXISTING` is only the
**conceptual** create-only shape; the **real** primitive is the dir-handle-relative form
of §2.3.3, because the path-string form does NOT close the parent-swap race — see §2.3.3.)
On the exists-error it bumps the counter suffix and yields the next candidate. So numbering
and the absolute no-clobber guarantee are the **same loop** — the directory's real state at
the instant of the exclusive publish decides, not a stale scan. (An optional cheap
`symlink_metadata` pre-check may skip obviously-taken low numbers as an optimisation, but
the **authority is always the kernel's exclusive dir-handle-relative publish**.)

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
  all FS calls use the **extended-length `\\?\` prefix** so ConvertIA itself can
  read/write long paths the engines were handed. **How `\\?\` is obtained `[DECIDED]`:**
  `std::fs::canonicalize` on Windows **already returns a `\\?\`-verbatim path** usable in
  syscalls (the §2.3.1 resolved-identity path is therefore already prefixed), so for any
  path we resolve there is nothing extra to add; for a path we *construct* (the §2.2
  numbered candidate) we **prepend the `\\?\` prefix manually** (an absolute path → a
  `\\?\`-verbatim path, the well-known Windows manual-prefix rule) before the syscall.
  **There is NO "dunce inverse"** — `dunce` only *strips* a verbatim prefix (for the
  §2.3.1 display/comparison form, §2.3.4); it never *adds* one. (The earlier "via the
  dunce crate's inverse" was a fiction — `dunce` has no prefix-adding API.) But a **final
  output path that
  the user/Explorer cannot then open** is still surfaced as a failure rather than a
  silent success. The check is: would the *user-facing* (non-`\\?\`) form **plus its
  NUL terminator** exceed `MAX_PATH` (260)? → fail clearly. `[CLARIFIED — P3.11]` The
  260 budget is **NUL-INCLUSIVE** (per this section's opening: "260 chars … drive +
  dirs + name + NUL"), so the usable user-facing length is **259** UTF-16 code units
  — a 260-unit path leaves no room for the terminator the Win32/CRT APIs append, so
  `check_path_limit` rejects `units + 1 > 260` (259 usable). Individual path
  **component** limit is **255** UTF-16 code units (NTFS) — also checked.
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
by `fs_guard::resolve_identity(path) -> io::Result<FileIdentity>` (**fallible**
`[CORRECTED 2026-07-07 — the P3.6 build]`: `canonicalize` fails on a path that does
not exist, so a missing source is a clean `Err` the §2.8 caller maps, never a panic
— the earlier `-> FileIdentity` was the infallible shorthand. The §2.3.2
retry-on-the-parent-when-absent is `is_safe_output`'s §2.3.3 OUTPUT-target concern,
not this — a frozen source exists at drop):

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
    `GetFileInformationByHandle` — obtained via **`winapi-util`**
    (`file::information(handle)` → `volume_serial_number()` / `file_index()`, both
    `u64`), the safe wrapper around that syscall already in the dependency graph
    (it is `same-file`'s engine, pulled by `walkdir`; §0.8 FS-guarantees row).
    `[CORRECTED 2026-07-07 — the P3.6 hard-stop]` The earlier "via
    `std::os::windows::fs::MetadataExt` `volume_serial_number()` / `file_index()`,
    available on recent Rust, else the `windows` crate" was factually wrong twice:
    those std methods are nightly-gated (`windows_by_handle`, rust-lang #63010 —
    E0658 on the pinned stable toolchain), and a raw `windows`-crate call would be
    an `unsafe` FFI site in the core, which the unsafe policy forbids outside the
    image-worker's allow-listed module. `same-file` itself exposes no Windows
    identity numbers (`Handle::dev()`/`ino()` are Unix-only), so `winapi-util` is
    the direct dependency. (The G29/`crate::platform` FFI-surface example lists NAMED
    `GetFileInformationByHandle` among the primitives the core MAY link raw until the
    2026-07-07 P3.12 ruling corrected them; `[CORRECTED — P3.9]` NO core path calls it
    raw: the §2.3.1 identity read here AND the §2.3.3 P3.9 dir-handle verify BOTH use
    `winapi-util`'s safe `information(&handle)` wrapper (P3.9 reads the ALREADY-OPEN
    dir handle via `winapi-util`'s `AsHandleRef for File`). The genuine raw per-OS
    handle FFI homed in `crate::platform` is `[re-cut by the P3.12 ruling]` the
    §2.1.2 **Windows-only** `windows-sys` extern set (the `FileRenameInfoEx`-class
    no-replace move, P3.14, + `GetDiskFreeSpaceExW`) — the Unix publish primitives
    (Linux `renameat2` / macOS `renameatx_np`, P3.12/P3.13) ride safe
    `rustix::fs::renameat_with(NOREPLACE)` with ZERO `unsafe` in the core — and none
    of it is `GetFileInformationByHandle`.) Equal triple ⇒
    same file ⇒ catches **hardlinks** and
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
   **Ordering with §2.7.1 subtree re-creation `[DECIDED]`:** this step presumes the parent
   directory **already exists**. For a **user-chosen-destination subtree** (`D/sub/dir/file`,
   §2.7.1) the parent may **not** exist yet — so the ordering is: **(a)** §2.7.1 create-only
   ancestor creation runs first (`mkdir` each missing ancestor under `D`, ancestor-by-
   ancestor); **(b)** the parent-dir handle is then opened on the **DEEPEST created dir**
   (the file's actual parent) and its identity verified per §2.7.1's full-final-dir
   link-safety; **(c)** the leaf is published **dir-handle-relative** against that verified
   directory (§2.3.3 below). So "resolve its parent" here means *the deepest created
   ancestor*, which by step (b) exists when this check runs — removing the "parent may not
   exist yet" ambiguity for the subtree path.
2. **Reject if** the resolved `final` (or its resolved-parent + leaf) has a
   `FileIdentity` equal to **any** frozen **source FILE**, **or** its resolved parent
   **resolves onto a frozen source FILE's resolved path** (e.g. the output dir is a
   symlink that resolves back onto a source file — a link chain cannot smuggle the write
   onto an original). `[CLARIFIED — P3.9]` This is resolved-identity / canonical-path
   **EQUALITY** with a source FILE, **not** an ancestor-prefix / "under a source's
   directory" test. **NOT** the dropped-root container itself: the frozen set holds **files only**
   (§0.6 invariant 4 — a dropped folder is recursed into files; the container directory is
   **not** a frozen-set source item), so landing beside-source **inside the dropped folder
   is the normal, correct case** and must **not** be rejected. The guard is "would this
   write resolve onto an *original file*?", **not** "is this path under a dropped folder?".
   "Writing beside a source would resolve onto the original" is exactly the rejected case
   (e.g. the output dir is a symlink back onto a source file).
3. On reject → **divert** to the §2.7 per-location fallback (Downloads/Documents or
   user-chosen), **never** proceed. The divert path is then re-checked (it too must
   pass `is_safe_output`).

Because step 2 also runs as part of the §2.1 exclusive-publish loop (§2.1.2), a link
that is created *between* the check and the write still cannot clobber a source: the
no-placeholder exclusive publish (Linux `renameat2(RENAME_NOREPLACE)` / macOS
`renameatx_np(RENAME_EXCL)` / common `link`+`unlink` fallback → `EEXIST`;
Windows `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` → `ERROR_ALREADY_EXISTS`)
fails on the existing (symlink) target and we re-pick.

**Parent-directory safety is made atomic via a directory-handle, not a path
`[DECIDED]`.** The `create_new` of the *leaf* is exclusive, but a separate TOCTOU
exists if the **parent** is swapped to a symlink (into a source tree) *between* the
parent canonicalisation (step 1) and the leaf create — a redirected-but-empty parent
would pass the leaf check yet land the file inside a source. To close it,
`is_safe_output` + §2.1's create operate **relative to an open parent-directory
handle**, not a re-resolved path string:

1. **Open the parent dir handle first** (`O_DIRECTORY` on Unix via `std::fs::File::open`
   on the dir; `NtCreateFile`/`CreateFile2` with a dir handle on Windows).
2. **Verify the open dir handle's identity** (`FileIdentity`, §2.3.1) is **not**
   inside the frozen set — by the **resolved-identity / canonical-path EQUALITY** of
   step 2 above ("resolves onto an *original file*?"), read from the OPEN handle,
   **NOT** an ancestor-path-prefix test (`[CLARIFIED — P3.9]`: the frozen set holds
   **files only** (§0.6 invariant 4), so a directory handle is "inside the frozen set"
   only if it resolves **onto** a source file; a literal path-prefix / containment test
   would wrongly reject the beside-source container step 2 explicitly permits).
3. **Publish the leaf *relative to that same open dir handle*** — and the publish is
   the **no-placeholder, create-only exclusive-rename of §2.1.2** (NOT an
   `openat(...O_CREAT|O_EXCL)` placeholder, which would create the rejected 0-byte
   `final`). Both per-OS publish primitives accept a **destination dir fd / handle**, so
   the dir-handle-relative form is the *same* primitive, just rooted at the verified
   handle:
   - **Unix:** `linkat(AT_FDCWD, tmp, dirfd, leaf, 0)` — or, where supported, the
     single-call dir-relative no-replace primitive **named per platform**: Linux
     `renameat2(olddirfd, tmp, dirfd, leaf, RENAME_NOREPLACE)` / macOS
     `renameatx_np(olddirfd, tmp, dirfd, leaf, RENAME_EXCL)` (both take a
     **`newdirfd`**). Fails `EEXIST` on collision → re-pick the next §2.2 variant. This
     is exactly the §2.1.2 link / `renameat2(RENAME_NOREPLACE)` (Linux) /
     `renameatx_np(RENAME_EXCL)` (macOS) create-only publish, rooted
     at the verified `dirfd` — **not** `openat(... O_CREAT|O_EXCL)`.
   - **Windows:** `NtSetInformationFile(tmpHandle, …, FileRenameInformationEx)` with a
     **`FILE_RENAME_INFORMATION_EX`** whose **`RootDirectory` = the verified parent dir
     HANDLE**, `FileName` = the relative `leaf`, and **`Flags` with `FILE_RENAME_REPLACE_IF_EXISTS`
     (0x1) OMITTED** (the Ex class uses a **`Flags` bitfield**, *not* the boolean
     `ReplaceIfExists` of the older non-Ex `FILE_RENAME_INFORMATION` — pick ONE struct; we
     use the **Ex** form for `RootDirectory`-relative resolution, so "no-replace" = clearing
     that flag bit, not setting a boolean false) → returns **`STATUS_OBJECT_NAME_COLLISION`**
     if `leaf` already exists → re-pick. This is the genuine **dir-handle-relative,
     create-only, no-placeholder** Windows publish: because the move resolves `leaf`
     *through the handle whose identity we just verified* (not by re-parsing a path string),
     the parent cannot be swapped between check and publish. **Bounded AV-retry on the
     publish `[DECIDED]`:** the publish itself may transiently fail with the NTSTATUS
     `STATUS_ACCESS_DENIED` / `STATUS_SHARING_VIOLATION` (AV/indexer holding a handle on
     `tmp` or `leaf`) — apply the **same bounded short-backoff retry as §2.1.2** (these are
     NTSTATUS codes, not the Win32 `ERROR_*` of `MoveFileExW`; map accordingly) before
     giving up to a §2.8 `WriteFailed`. (Plain path-string `MoveFileExW` re-resolves `final`
     by path at publish time and so does **not** close the parent-swap race on Windows;
     `FileRenameInformationEx` with `RootDirectory` is what closes it.)

So beside-source and divert writes both use a **dir-fd/handle-relative, create-only
exclusive publish**; the parent's identity is checked once on the handle, and the leaf
is published through it — neither the parent nor the leaf can be link-redirected in the
gap. `fs_guard::atomic_publish(parent_handle, tmp, leaf)` encapsulates the per-OS form.

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
- **The key has NO destination component, so in v1 the re-run verdict is
  destination-INDEPENDENT `[DECIDED]`.** Signal 1 (the in-session ledger) keys on
  `(source_identity, target_format, settings)` only; changing the destination (C5)
  cannot create or clear a ledger hit. A C5 `set_destination` change therefore
  re-evaluates **only** the destination-dependent preflight (free space on the new
  volume, §2.14.4 / §1.10) and **leaves `rerun` unchanged** — it is carried through
  from the C4 verdict, never recomputed by destination. A future destination-aware
  signal (per-destination disk-presence) would require the deferred cross-session
  ledger and is **[DEFER: post-v1]** (see §2.5.3 fallback note).

### 2.5.2 Detecting "this exact conversion already produced output" `[DECIDED]`

Detection is **best-effort** and, in v1, rests on **one authoritative signal** — the
in-session ledger — with disk presence demoted to a non-firing corroborator (no DB,
honoring §7.4's "persist nothing / session-only" lean — see *fallback* for the
cross-session limit):

1. **In-session run ledger (the sole authority that fires the prompt) `[DECIDED]`.**
   Within the **current app session**, `crate::run` keeps an in-memory
   `HashSet<EquivKey>` of conversions already completed this session (cleared on
   quit; nothing written to disk, §7.4). A second identical drop in the **same
   session** hits the ledger → definite equivalence → the prompt fires. This is the
   **only** signal that, on its own, fires the re-run prompt in v1.
   - **Vanished-output edge `[DECIDED — accept the semantic]`:** the ledger fires from
     the in-memory hit **alone**, with no disk corroboration, so if the prior output was
     **deleted/moved between the two runs in the same session**, the prompt still fires
     and choosing **Skip** yields *no* output. This is **accepted and documented**: the
     ledger answers "you asked for this exact conversion already this session" (which is
     true), and **Skip is honoured as the user's deliberate choice**; **Make a fresh
     copy** re-creates it via ordinary §2.2 numbering. (We deliberately do **not**
     corroborate signal 1 with a disk-presence check before firing — that would couple
     the session-identity signal to disk state for a rare case; the safe default is Skip
     and the user can pick fresh-copy. Disk-presence corroboration of a *vanished* output
     is `[DEFER: post-v1]` with the cross-session ledger.)
   - **Changed-destination edge `[DECIDED — accept the semantic]`:** because the EquivKey
     has **no destination component** (§2.5.1) and the in-session ledger is the **sole**
     firing authority, converting a set beside-source and then **re-dropping the same set
     to a freshly-chosen folder** still fires the re-run prompt — even though that chosen
     destination holds **no** prior output — and **Skip** then produces nothing *there*.
     This is within best-effort tolerance (the same family as the vanished-output edge: the
     ledger answers "you asked for this exact conversion this session", which is true) and
     is accepted in v1; the destination-aware re-run signal that would distinguish it is the
     same **`[DEFER: post-v1]`** item (needs the cross-session/destination-aware ledger).
2. **Expected-output presence (corroborator only — never fires alone) `[DECIDED]`.**
   ConvertIA writes **deterministic** names, so a prior identical run's output sits
   exactly where this run's first candidate (`stem.ext`, §2.2) would go. **But an
   existing same-named file is not, by itself, re-run evidence**: nothing on disk
   distinguishes "the output of a prior identical run" from "an unrelated
   pre-existing file that merely shares the name" — and the SSOT requires these two
   cases **not** be conflated (an unrelated collision → silent numbering, §2.2;
   only a *detected* re-run → prompt). Therefore disk presence **must not fire the
   prompt on its own**: across sessions (where the ledger is empty) an existing
   same-named file is an **ordinary collision → silent next-free-variant numbering**,
   never a re-run. Presence is used only to *corroborate* signal 1 within the same
   session (it cannot upgrade a non-ledger hit into a re-run). The qualifier
   "consistent with a prior identical run" is, on disk alone, unmechanisable — which
   is exactly why signal 1 is the sole authority. (A sound cross-session signal would
   require the deferred cross-session ledger; see *fallback* and §7.4.)

A batch-level prompt fires when **any** item in the batch is flagged equivalent
**by signal 1 (the in-session ledger)**.

**When detection runs (ordering) `[DECIDED]`.** Re-run equivalence is computed during
**C4 `plan_output`** and returned in `OutputPlanPreview.rerun` (§0.4.1) — so the UI
enters the **RerunPrompt** state (§5.2 state 6) **from the C4 response, before
Convert is pressed**, not as a separate round-trip after C6. C6 `start_conversion`
then carries the user's `RerunDecision` (Skip / FreshCopy). (§1.0's flow reflects
this: re-run detection sits at the §1.8 output-planning step, which C4 drives.)
**Concurrent identical batch edge `[DECIDED: accept best-effort degradation]`:** if an
identical batch is *running* in the same session when C4 queries the ledger, the
in-flight batch's `EquivKey`s are added to the ledger only at *its* completion, so the
second batch may miss the equivalence and fall through to §2.2 **silent
next-free-variant numbering** — a silent extra numbered copy the prompt would otherwise
have offered to skip. We **do not** reserve in-flight `EquivKey`s at batch start in v1
(it would add cross-batch locking for a rare race); the residual is an **acceptable
best-effort miss** within the SSOT's stated tolerance (never an overwrite, §2.1).
Reserving in-flight keys is **[DEFER: post-v1]** alongside the cross-session ledger.
It is **one** prompt for the whole batch (SSOT: "one plain batch-level prompt"),
surfaced as the §5.2 *re-run prompt* state with `Skip` (default, focused) /
`Make fresh copies`. The prompt's strings are UI-chrome (§5.7), but the **decision
semantics** (skip-default; fresh-copy → ordinary numbering) are owned here.

### 2.5.3 Best-effort fallback (never overwrite) `[DECIDED]`

When ConvertIA **cannot** determine equivalence — the prior output was **renamed or
moved** (so the deterministic name is free again), or this is a **new session** and
the ledger is empty — it **does not** guess. It falls
through to §2.2 **silent next-free-variant numbering**. The invariant the SSOT
pins: the *failure mode of detection is a harmless extra numbered copy, never an
overwrite*. This is acceptable precisely because §2.1's exclusive-create makes
overwrite impossible regardless of what §2.5 concludes — §2.5 only decides *prompt
vs silent-number*, never *number vs overwrite*.

> **`[DECIDED]` cross-session ledger = NOT in v1 (session-only).** §2.5 stays
> session-only (the in-memory ledger, signal 1's sole authority §2.5.2); the harmless
> extra numbered copy across sessions is within the SSOT's stated best-effort
> tolerance, and "persist nothing" is honored (§7.4). A future on-disk
> `EquivKey`-hashes-only record (which would restore a sound cross-session re-run
> signal) is **[DEFER: post-v1]** — if added it must store **only opaque hashes,
> never paths/content** (privacy invariant §2.11). (Adopting the standing [REC]: do
> not persist in v1.)

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

ConvertIA has **two kinds of temp** with **different homes** — the split and the
volume rule are owned by §2.14.2 (referenced here, not re-decided):

- **Kind-2 engine working files** (LibreOffice profile, FFmpeg internal temp, etc.)
  live under the **central per-run scratch dir** named with the `RunId` (§7.1):
  `…/convertia/scratch/<InstanceId>.<pid>/run-<RunId>/` under the §2.14 scratch root
  (the `scratch/` component the §2.6.3 sweep glob `convertia/scratch/<*>.<*>/run-*` and
  the `crate::run` P3.21 assembly both use — the full path is
  `<app_local_data_dir>/convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`).
- **The kind-1 publish temp (`*.part`)** does **not** live in the central scratch
  root — its location is **deferred entirely to §2.14**, which puts it on
  **`final`'s volume** (the destination dir) so the §2.1 publish is a true
  intra-volume atomic rename even in the common beside-source case (where the
  central scratch root would frequently be on a *different* volume → an `EXDEV`
  cross-volume move on the **common** path, defeating §2.1). Per §2.14.1 the publish
  temp is a uniquely-named **sibling file** in the destination dir, e.g.
  `…/<dest_dir>/.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`, **not** a subdir, so
  the startup sweep (§2.6.3) can opportunistically remove a stale one when a later run
  next writes there — resolving its exact owning lock cross-instance from the embedded
  `InstanceId`+`RunId`.
- **`InstanceId`+`RunId` encode ownership** in both temp names, so a cleanup sweep can
  tell *its own* temps from a *concurrent instance's* temps (and never deletes a live
  foreign instance's in-progress `.part`). The RunId model and its
  uniqueness/liveness semantics are §7.1's to define; §2.6 *requires* it to be (a)
  unique per run and (b) liveness-checkable (so a stale central scratch dir from a
  dead run is distinguishable from a live one — see 2.6.3).

### 2.6.2 Cleanup triggers `[DECIDED]`

`crate::run::cleanup_item` / `cleanup_run` remove run-owned temps on every exit path:

| Trigger | Action |
|---------|--------|
| **Item success** | **single-call path** (Linux `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)` / Windows `MoveFileExW`): `tmp` was consumed by the publish — nothing to remove. **`link`+`unlink` fallback path** (either Unix OS where the single-call primitive is unavailable): the publish `link`ed `tmp→final`, so `unlink(tmp)` removes the `*.part`; if that unlink fails, the residual is reclaimed by the §2.6.4 sweep (annotated, not an item failure). |
| **Item failure** (engine error, corrupt, etc.) | remove that item's `tmp`. |
| **Cancel** (user) | §1.7 kills the engine group and, on a **bounded** confirm-wait, removes the killed item's `tmp`; **already-finished items are kept** (SSOT). **If the group-kill confirm-wait times out** (a wedged descendant still holding the `*.part`), reclamation of that publish temp is **deferred to the §2.6.4 sweep** and surfaced as a `CleanupResidue` on the Cancelled item (§2.6.4 case 3) — i.e. tmp is *not* unconditionally removed here. |
| **Out-of-disk mid-write** | remove the partial `tmp`; report `OutOfDisk` (§2.8); **batch continues** (SSOT). |
| **Run end (any reason)** | remove the now-empty central `run-<RunId>/` dir **and** leftover publish temps in the run's **RECORDED `final_dir` set** — the union of **every distinct `final_dir` actually used this run**, tracked in memory as outputs are planned/written. This is **not** just the dropped/destination roots: it **includes late-divert targets (§2.7.2) and cross-volume intermediates (§2.14.3)**, which can land in dirs that are neither a drop root nor the chosen destination. **CRITICAL — own-prefix scope, never a bare `*.part` glob `[DECIDED]`:** a RECORDED `final_dir` can be **shared across concurrent instances** (the beside-source-into-the-same-folder scenario §2.6.1/§2.6.3 guard), so run-end cleanup removes **only this run's own temps by exact prefix** — `.convertia-<thisInstanceId>-<thisRunId>-*.part` — in each recorded dir, and **never** a bare `*.part` / `.convertia-*.part` glob (which would delete a concurrent foreign instance's **live** in-progress `.part`, violating the SSOT *"cleanup never removes another instance's in-progress file"*). For any **non-matching** `.convertia-*.part` encountered in a recorded dir, apply the §2.6.3 per-file **"held lock ⇒ keep"** guard so a foreign live temp is never deleted (a dead foreign run's residue may be opportunistically reclaimed under that guard, but never a live one). (Recording the actual `final_dir` per item as it is written is what makes run-end cleanup enumerate every dir a `*.part` could have been written to; the §2.6.3 opportunistic/startup sweep is the post-crash backstop.) |
| **Next app start** | sweep stale central `run-<RunId>/` dirs from prior runs (§2.6.3); destination-resident `*.part` from a *crashed* prior run are reclaimed opportunistically by a later write into that dir, not by the startup sweep (§2.6.3 limitation). |

Removal restores free space to "roughly what it was before the run" (SSOT) — temps
are the only thing ConvertIA adds to disk besides the final outputs, and successful
finals are intended; failed/cancelled items leave nothing.

### 2.6.3 Startup sweep — never touch a live instance's temp `[DECIDED]`

On startup (§7.2 sequence) `crate::run::sweep_stale`:

1. Lists run dirs across **ALL instance dirs** — the glob is
   `convertia/scratch/<*>.<*>/run-*` (every `<InstanceId>.<pid>` dir, not just this
   instance's), under the **central scratch root** (kind-2 working files), so a crashed
   *foreign* instance's stale run dirs are reclaimable too.
2. For each, checks **liveness** via §7.1's mechanism — **the held lock is the SOLE delete
   gate** (an mtime/PID is never a delete predicate on its own) — recommended: an **advisory
   lock file** `run-<RunId>/.lock` held with an OS lock for the run's lifetime, probed
   with a **NON-BLOCKING try-lock `[DECIDED]`** so the sweep never hangs on a live
   instance (the app must stay responsive): **Unix `flock(LOCK_EX | LOCK_NB)`** or
   **`fcntl(F_SETLK)`** (`F_SETLK` is already non-blocking; bare `flock` **without**
   `LOCK_NB` would *block* until the holder releases — wrong here); **Windows
   `LockFileEx` with `LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK`** (bare
   `LockFileEx` **without** `LOCKFILE_FAIL_IMMEDIATELY` blocks — wrong here). Interpretation:
   **would-block / immediate-fail ⇒ the lock is still held ⇒ LIVE ⇒ left untouched**;
   **immediate acquire ⇒ free/stale ⇒ DEAD/crashed run ⇒ removed** (and the sweep
   immediately releases the lock it just took). The working files of a dead run are the
   discardable artifacts the SSOT says are "cleaned up on next run".
   **Create-then-not-yet-locked window `[DECIDED]`:** there is a tiny window where a brand-new
   live run has **created its `run-<RunId>/` dir but not yet acquired the `.lock`**, during
   which the sweep would see a lockless dir and could wrongly delete it. Two guards close it:
   (a) the **lock-before-population ordering invariant** (§2.6.3 below / §2.14.1: a run creates
   `run-<RunId>/`, acquires + OS-locks `.lock`, and only then writes any content — so a
   *populated* dir always has a held lock); and because a freshly-created **empty** dir can
   still precede its lock, (b) the sweep **skips any lockless run dir whose mtime is within a
   short grace window** (created in the last few seconds) and reclaims it only on a later sweep
   if it is still lockless and stale. A lockless **stale** dir (mtime well past the grace
   window, no held lock) is dead and reclaimed; a lockless **very-recent** dir is left for next
   time. This keeps "held-lock ⇒ keep" authoritative while never racing a just-starting run.
3. This is what makes the SSOT promise *"temp artifacts are owned per-run so cleanup
   never removes another instance's in-progress file"* concrete: liveness is by
   **held-lock — the single authoritative predicate** — not by guessing from
   timestamps and **not** by the PID embedded in the dir name (§7.1.2). The PID is a
   human-readable label / fast pre-filter only; it is **not** the liveness test
   (PIDs are reused → a dead run's PID may belong to a live unrelated process). The
   held advisory lock is the one predicate both §2.6 and §7.1.2 defer to.

**Destination-resident publish temps — the honest limitation `[DECIDED]`.** The
kind-1 `*.part` publish temps live in the *destination* dirs (§2.14.1), **not** the
central scratch root, and §7.4 does **not** persist the set of destination roots a
prior run wrote to. So a post-restart sweep **cannot enumerate** where stale `*.part`
files might be — the central-scratch sweep above will not find them. ConvertIA
therefore reclaims a destination-resident publish temp at exactly two moments:
**(a)** at run-end and same-session retry (the destination roots are known
in-memory), and **(b) opportunistically**: whenever any *later* run is about to write
into a destination dir, it first removes any sibling stale `.convertia-*.part` whose
owning run is **dead** (same liveness check as step 2, applied per-file). **Cross-instance
liveness `[DECIDED]`:** because the `.part` filename embeds **both `InstanceId` and
`RunId`** (§2.14.1), the per-file check resolves the **exact** owning lock —
`convertia/scratch/<InstanceId>.*/run-<RunId>/.lock` — and is therefore **correct across
instances** (instance A about to write into a dir does **not** delete a **live** instance
B's in-progress `.part`, because B's lock is held). A lock that is **held** ⇒ live ⇒ the
`.part` is **kept**; a lock that is **free, stale, OR entirely absent** ⇒ dead ⇒ the
`.part` is reclaimable. (Only a currently-held lock blocks reclamation; an absent lock is
not "uncertain" — it is dead.)
**Pre-RunId probe residue — `InstanceId`-only liveness `[DECIDED]`.** The §2.7.2 C4
writability probe runs **before** any `RunId` exists, so its leftover is named
`.convertia-<InstanceId>-probe-<rand>.part` (no `RunId`, no `jobId`). The per-file sweep
recognises this shape and resolves liveness by **`InstanceId` alone**: if **any** lock
under `convertia/scratch/<InstanceId>.*/` is currently held the owning instance is alive ⇒
**keep**; if no live lock exists for that `InstanceId` the instance is dead ⇒ **reclaim**.
This never consults a `run-<RunId>/.lock` (a probe never minted one). A live foreign
instance's probe residue is therefore never deleted while that instance is alive, exactly
like the run-owned `.part` case.
**Lock-before-part ordering invariant — what makes "absent ⇒ dead" SAFE `[DECIDED]`.**
The "absent lock ⇒ dead ⇒ reclaimable" rule is correct **only because of a guaranteed
ordering**: a run's `run-<RunId>/.lock` is **created and OS-locked BEFORE the run writes
its first `.part`** (and held for the run's whole lifetime, released only at run-end /
process exit). So there is **never** a window where a live in-progress `.part` exists while
its owning `run-<RunId>/.lock` is absent — if instance B has written a `.part`, B's lock
provably already exists and is held, so a concurrent instance A sweeping that dir finds the
lock **held** and **keeps** B's live `.part`. Without this ordering, A could see a
not-yet-created lock as "absent ⇒ dead" and delete B's live `.part`, violating the SSOT
*"cleanup never removes another instance's in-progress file"*. The ordering is the
**premise** the conclusion rests on; it is established by `crate::run` at run start (mint
`RunId` → create `run-<RunId>/` → acquire the `.lock` → only then begin writing `.part`s)
and is a §6 property-test target.
This is why the publish temp is a uniquely-named **file**
(not a subdir) named with the `InstanceId`+`RunId` — it makes the opportunistic same-dir
sweep cheap, cross-instance-safe, and lock-addressable without scanning every instance
subdir.
A publish temp in a destination dir **never revisited** by a later run can persist
until the user deletes it; this residual case is surfaced honestly per §2.6.4 rather
than promised away. **SSOT reconciliation `[DECIDED]`:** the "free space returns to
roughly pre-run" promise holds **fully** on graceful failure/cancel **in the normal
case** (the engine group exits within the §1.7 confirm-wait and run-end cleanup reclaims
every temp), and on the next write into that destination. **Two carve-outs where a single
destination-resident `*.part` (≈one output size) can linger** until opportunistic
reclamation: **(1) a true crash** (no chance to run run-end cleanup); **(2) a graceful
cancel where the §1.7 bounded group-kill confirm-wait TIMED OUT on a wedged descendant**
(§2.6.2 Cancel row / §2.6.4 case 3) — there the publish-temp reclamation is **deferred to
the §2.6.4 sweep** and the Cancelled item surfaces a `CleanupResidue`, so a `*.part` can
linger past the cancel. Both are **within the SSOT's stated best-effort cleanup tolerance**
(the SSOT promises best-effort temp cleanup, not a guaranteed sweep when a descendant is
wedged or after a crash) and are surfaced honestly (§2.6.4), never promised away.

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
- **An item was *cancelled* and its publish temp was not removed in the §1.7 bounded
  group-kill confirm-wait** (a wedged descendant still holds the `*.part`): the Cancelled
  item **carries a `CleanupResidue`** naming the deferred path, the temp is reclaimed by
  the §2.6.3/§2.6.4 sweep, and the §2.8.2 Cancelled summary gets the **"With residue"** tail
  ("Some temporary files may remain — see details."). This closes the gap where the §2.6.2
  "Cancel" row used to imply unconditional removal — a wedged-cancel residue is tracked and
  surfaced, never silently dropped.

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

   **Subtree directory-creation mechanism `[DECIDED]` (owner: §2.7 / §1.8).** Re-creating
   `D/sub/dir/` is more than "the relative path is preserved" — the intermediate dirs must
   be created safely, **before** the engine writes its `out_tmp` there (§2.14.1), and the
   creation itself must honour the §2.3 link-safety invariant:
   - **Create-only, ancestor-by-ancestor.** Each missing ancestor under `D` is created
     **create-only** (`mkdir`, never `mkdir -p`-that-silently-accepts-an-existing-file): a
     `create_dir` that races another creator and gets `AlreadyExists` re-checks that the
     existing entry **is a directory** and continues; any other error fails the item.
   - **Non-directory collision fails clearly.** If an ancestor path `D/sub` already exists
     as a **non-directory** (a regular file / symlink-to-file occupies the name), the item
     **fails with a clear §2.8 error** (an `Unwritable`/`Io`-class kind), never silently
     overwriting or diverting around it.
   - **Full-final-dir link-safety, not just the leaf.** The §2.3.3 link-safety check is
     extended to the **whole final directory** `D/sub/dir` (canonicalise/identity-check the
     **deepest** created/existing dir), not only the immediate leaf parent — a symlinked
     `D/sub` could redirect the write into a frozen source tree (§2.3 forbids writing onto
     a source), and checking only the leaf parent would miss an ancestor redirect. The
     §2.3.3 parent-handle for the exclusive publish is taken on the **deepest created dir**
     (the file's actual parent), so the publish is dir-handle-relative against the verified
     directory.
   These steps run in §1.8 path planning / §2.7 before the engine spawn; a failure at any
   ancestor surfaces as a per-item §2.8 failure (batch continues, §1.9), never a partial
   silently-wrong tree.

### 2.7.2 Per-location writability & ephemerality classification `[DECIDED]`

For each source, §1.8 classifies its **intended** output location via
`fs_guard::location_status(dir)`:

- **Writable test:** create-and-immediately-remove a **throwaway probe file** via
  `create_new` in the target dir — this only confirms the directory **accepts a create**.
  **Note: this is NOT the §2.1 publish primitive.** §2.1.2's no-placeholder publish
  **never** uses `create_new` at the final path (it is the exclusive *rename* of the
  finished temp); the probe's `create_new` is on a disposable probe path, not the output.
  Do **not** wire the probe and the publish to the same helper. Failure
  (`PermissionDenied`, `ReadOnlyFilesystem`, network errors) → **unwritable**.
  *Recommended:* probe lazily and cache per-directory within the run to avoid probing
  every file in a 10 000-file batch in the same folder.
  - **Probe-cleanup-failure handling `[DECIDED]`.** If the probe file is *created*
    (so the dir is writable) but its **removal fails**, the verdict is **writable**
    (the create succeeded — that is the test) and the leftover probe file is **not**
    cause to divert. **The probe runs at C4 (§1.8/§1.10), BEFORE the RunId is minted
    (§7.1.2: RunId is minted at C6), so it CANNOT carry a `RunId` and uses a distinct
    pre-RunId name keyed on `InstanceId` only: `.convertia-<InstanceId>-probe-<rand>.part`.**
    The §2.6.3 per-file sweep reclaims this `InstanceId`-only probe residue via **InstanceId
    liveness** (any live lock under `convertia/scratch/<InstanceId>.*` ⇒ the instance is
    alive ⇒ keep; no live instance lock ⇒ dead ⇒ reclaim) — **not** a `run-<RunId>/.lock`
    that never existed for a probe. The failure is logged locally (§7.5) only. We never
    divert *solely* because probe-cleanup failed.
  - **The per-directory writability cache is a planning *hint*, not a commitment
    `[DECIDED]`.** A location can flip read-only *between* the probe and the actual
    write (USB pulled, share dropped, permission changed mid-run). When the real
    §2.1 publish then fails for a writability reason, ConvertIA **re-triggers the
    §2.7 per-location divert for that item** (treat the just-failed location as
    unwritable, divert to the §2.7.3 target, re-run the §2.1 publish there) **before**
    reporting any failure. Only if the divert target *also* fails to accept the write
    does the item report `WriteFailed` (§2.8). So a post-probe read-only flip is a
    **late divert**, not an immediate item failure — consistent with §2.7's
    divert-unwritable-locations intent. (A non-writability error — e.g. `OutOfDisk` —
    is **not** a divert trigger; it fails per §2.8 / §1.10.) **The late-divert publish
    re-runs the full safety chain on the divert target** — because the up-front per-volume
    free-space verdict (§2.14.4) and the §2.2.3 path-limit check were computed for the
    **original** beside-source path, which can be a **different volume and a different
    absolute-path length** than the divert destination (Downloads/Documents):
    - §2.3.3 `is_safe_output` (the divert dir must not resolve into the frozen source set),
    - §2.2.3 **path-limit re-checked against the divert destination's full absolute path**
      (fail the item `PathTooLong` if it would exceed the OS limit — never silently
      truncate),
    - the **§2.14.4 per-physical-volume free-space check re-run against the divert
      destination's volume** (fail the item `OutOfDisk` if it would not fit — never assume
      it fits because the original volume did),
    all **before** its §2.1 exclusive publish. A failed re-check fails that **one** item
    clearly (`PathTooLong` / `OutOfDisk`, §2.8) while the batch continues — a late divert
    never skips the link-safety / path-limit / free-space checks just because it is a
    fallback (SSOT Principle-5: guarantees apply **identically on the divert path**).
- **Ephemeral test:** is the dir inside a **known-ephemeral OS temp location**?
  - Win: under `%TEMP%` / `%TMP%` / `GetTempPathW`.
  - macOS: under `$TMPDIR` (per-user `…/T/`), `/tmp`, `/var/folders/…`.
  - Linux: `$TMPDIR`, `/tmp`, `/var/tmp`, `/run/user/<uid>` (XDG runtime).
  Writing a *result* into a place the OS may purge silently loses the user's output
  → treated like unwritable → divert. (Reading a source from there is fine; only the
  *output* diverts.)
- A **read-only USB / network share** surfaces as unwritable by the probe.
- **No-atomic-publish-primitive test (FAT/exFAT and similar) `[DECIDED]`.** A
  destination filesystem may **accept a create** (so it passes the writable probe) yet
  offer **no mechanised create-only / atomic no-clobber publish primitive at all** — the
  load-bearing case is **FAT32/exFAT** (the spec's canonical portable-USB destination,
  §2.14.2). There, the §2.1.2 single-call no-replace primitive is **unavailable on both
  OSes** (Linux `renameat2(RENAME_NOREPLACE)` → `EINVAL`; macOS exFAT does **not**
  advertise `VOL_CAP_INT_RENAME_EXCL`) **and** the `link`+`unlink` fallback **cannot run**
  because FAT/exFAT have **no hardlink support** (`link()` → `EPERM`/`ENOTSUP`). So neither
  the no-clobber nor the atomic-publish half of §2.1 has an implementation there. **The
  probe therefore additionally classifies the destination's filesystem:** when the volume
  is detected as one lacking BOTH `RENAME_NOREPLACE`-class no-replace rename AND hardlinks
  (FAT/exFAT-class; detected via the OS filesystem type / `statfs`-class query, or by a
  one-shot capability probe in the probe dir — a no-replace-rename attempt that returns
  `EINVAL`/unsupported AND a `link()` attempt that returns `EPERM`/`ENOTSUP`), it is
  treated as **"cannot guarantee atomic no-clobber here" → a per-location DIVERT trigger**
  (exactly like the unwritable case, carrying **`DivertReason::NoAtomicPublish`**, §0.6):
  the item's output **diverts to the §2.7.3 target** (Downloads/Documents on the
  hardlink-capable **system disk**), where the full §2.1 exclusive-publish chain holds. This is **Unix-only** — Windows' `MoveFileExW`-without-
  `MOVEFILE_REPLACE_EXISTING` (§2.1.2) is a true create-only move on FAT/exFAT too, so a
  Windows FAT/exFAT destination is **not** diverted. The §2.7.3 divert target is itself
  re-run through `location_status` (including this new test), so a divert can never land
  the output on another FAT/exFAT volume; if the only writable target is FAT/exFAT-class,
  the item **fails clearly** (`WriteFailed`, §2.8) rather than silently dropping the
  no-clobber/atomic guarantee. (Windows-FAT and any divert-impossible Unix-FAT case are the
  only ways a FAT/exFAT destination is reached, and the former is safe by construction.)
  Cross-ref §2.1.2 (the third-fallback case), §2.14.2 (USB canonical destination).

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
- **The resolved divert target is itself subjected to the §2.7.2 ephemeral +
  writability test `[DECIDED]`.** A divert target could *also* be ephemeral or
  unwritable (a kiosk that redirects Downloads into a purgeable location; the same
  pulled USB; a restricted Documents). So before diverting, `Downloads`/`Documents`
  (or the user-chosen root) is run through the **same `location_status` check**: if it
  resolves to an **ephemeral or unwritable** place, the item **fails clearly with
  `WriteFailed` (§2.8)** rather than diverting an output into a place the OS may purge.
  (The §2.3.3 `is_safe_output` link-safety + §2.2.3 path-limit run on the divert target
  too, per §2.7.5 — the divert is never a degraded path.)

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

A Rust enum in `crate::outcome`, each variant a **stable kind** carried over IPC
(§0.4 owns the wire shape; §2.8 owns the *set* and their strings). Every engine /
FS / detection failure **must** map to exactly one of these — there is no "other /
unknown" that leaks a raw error to the user (an unmapped internal error becomes
`InternalError` with a generic calm message, §2.13).

```rust
#[derive(Serialize, specta::Type)]   // generated into bindings.ts; §0.4.3 ErrorKind is its wire mirror
#[serde(rename_all = "camelCase")]
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
    TooManyCollisions,  // §2.1.2/§2.2 — the ~10,000-variant no-clobber cap was exhausted (a degenerate dir)
    EngineCrash,        // subprocess killed by signal / nonzero abnormal exit (§1.7/§2.12)
    EngineHang,         // exceeded the §1.7 timeout, killed (§2.12)
    EngineError,        // subprocess clean nonzero exit w/ classifiable stderr (§3.5)
    PlatformUnavailable,// patent-gapped on this platform (§3.4) — honest "unavailable here"
    QuarantinedByOs,    // macOS Gatekeeper quarantined a bundled engine sidecar so it can't spawn (§7.2.3) — distinct from EngineMissing/BundleDamaged
    CleanupResidue,     // item failed AND its partial couldn't be removed (§2.6.4)
    InternalError,      // catch-all for an unexpected internal fault (§2.13), no trace shown
    // ── run/app-level (§2.13); surfaced via app://fault, not a per-item row ──
    EngineMissing,      // a required bundled engine is absent/unrunnable at startup (§7.2)
    WebviewFault,       // the WebView core disconnected / failed to load (§2.13/§5.8)
    BundleDamaged,      // the app bundle/resources failed their integrity check (§7.2)
    // ── pre-flight (NOT carried as an IpcError; mirror-only for drift-lock) ──
    MixedDrop,          // >1 source format in one drop — pre-flight refusal (§1.3); chrome string §5.
                        //   NO §2.13 producer: it is the CollectedSet::Mixed SUCCESS return from C1
                        //   (§0.6) driving the §5.2 state-9 refusal — listed here ONLY to keep the
                        //   enum byte-identical to the §0.4.3 wire mirror (do not search §2.13 for it).
}
```

A `ConversionError` carries the kind, the **owning source path** (for the summary),
optional **detected-type detail** (for `UnsupportedType`), and an optional
**residue path** (for `CleanupResidue`). It deliberately carries **no** stack trace,
no Rust `Debug` of the underlying error, no engine command line (that goes to the
local log §7.5 if enabled, never to the user — SSOT "no stack traces").

The **item-level** kinds are reported as a per-item `Failed` row and the batch
keeps going (§1.9); the **run/app-level** kinds (`EngineMissing`,
`WebviewFault`, `BundleDamaged`) are not per-item outcomes — they travel over the
`app://fault` path (§0.4.2, §2.13). **`MixedDrop` is neither item-level nor app://fault** —
it has **no IpcError producer at all**: it is the `CollectedSet::Mixed` SUCCESS return from C1
(§0.6), the pre-flight refusal (§1.3) surfaced with §5 chrome (state 9). It appears in the
enum only as the byte-identical wire mirror (the catalog below covers the
item-level kinds; the app-level kinds carry §5/§7.2 chrome strings, not §2.8.2
rows).

### 2.8.2 The message catalog `[DECIDED]`

The **exact canonical English strings**. One row per kind. `{x}` are runtime
substitutions filled by `crate::outcome` (the type name, the path, the size). Tone:
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
| `TooManyCollisions` | **"There are already too many files with this name in that folder, so this one couldn't be saved. Try a different folder."** | — | the §2.1.2/§2.2 no-clobber numbering cap (~10,000 variants) was exhausted; a degenerate destination directory. |
| `EngineCrash` | **"Something went wrong while converting this file, so it was skipped."** | — | subprocess crash; no trace shown. Detail goes to §7.5 log only. |
| `EngineHang` | **"This file took too long to convert and was stopped."** | — | §1.7 timeout. |
| `EngineError` | **"ConvertIA couldn't convert this file."** | — | clean nonzero exit; generic calm fallback. |
| `PlatformUnavailable` | **"This conversion isn't available on {platform} because the required format support can't be included here."** | `{platform}` | the §3.4 honest per-platform gap; SSOT v1-DoD exception 1. |
| `QuarantinedByOs` | **"macOS is blocking one of ConvertIA's built-in tools with a security check. Open System Settings → Privacy & Security and choose "Open Anyway", then try again."** | — | macOS Sequoia per-sidecar quarantine — a bundled engine couldn't spawn because Gatekeeper quarantined it (§7.2.3); distinct from a missing/corrupt engine. |
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

**`OutcomeMsg` — the surfaced per-item string (defined here; §0.6 `ItemResult.reason`
references it).** The §0.6 `ItemResult.reason: Option<OutcomeMsg>` is **either** a §2.8
failure string **or** a §2.9 lossy note. It is the *resolved, ready-to-show* line (so
the summary needs no second lookup), produced by `crate::outcome` from the kind + its
substitutions:

```rust
/// A surfaced one-line outcome for one item (§0.6 ItemResult.reason). Carries the
/// stable discriminant so §5 may re-localise (§2.10) AND the resolved English line.
#[derive(Serialize, specta::Type)]            // wire type — see specta note below
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
enum OutcomeMsg {
    Failure { kind: ConversionErrorKind, text: String },  // §2.8.2 catalog row, substituted
    Lossy   { kind: LossyKind, text: String },            // §2.9.1 note, substituted
    Skipped { reason: SkipReason, text: String },         // §0.6 SkipReason — a pre-flight
                                                          //   ineligible (UnsupportedType/Empty/
                                                          //   Unreadable/Unrecognized). A skip is
                                                          //   NOT a failure: it rides a skip-shaped
                                                          //   variant so a consumer pattern-matching
                                                          //   OutcomeMsg can tell skip from fail
                                                          //   WITHOUT also reading ItemResult.state
                                                          //   (§0.6 JobState distinguishes them and
                                                          //   §1.12 Totals counts them separately —
                                                          //   "must not be conflated").
}
```

`text` is the canonical English from the catalog above (§2.8.2) or the §2.9.1 note
table, with `{x}` substitutions already applied; `kind` lets §5 swap in a localised
string later without re-deriving the outcome.

**Wire-type derivation — both `OutcomeMsg` and `ConversionErrorKind` derive
`specta::Type` `[DECIDED]`.** `OutcomeMsg` crosses the boundary inside
`ItemResult.reason: Option<OutcomeMsg>` (§0.6), which rides the `RunFinished` Channel
payload and the C8 return, and it carries `ConversionErrorKind` in its `Failure`
variant. tauri-specta generates `bindings.ts` only from types deriving `specta::Type`;
if these two did not, codegen would fail or fall back to `any` for
`ItemResult.reason` — violating the platform **no-`any`** rule. Therefore **both
`OutcomeMsg` and `§2.8.1 ConversionErrorKind` (and `§2.9 LossyKind`) derive
`specta::Type` and are registered in `collect_types![]`** (alongside the §0.4.3
`IpcError`/`ErrorKind`). The §06 bindings-drift check (§0.4.5) **covers these types
too**, so a change to the §2.8 taxonomy or the lossy catalog regenerates
`bindings.ts` and fails CI if stale. (`ConversionErrorKind` is the §2.8-owned full
set; `§0.4.3 ErrorKind` is its byte-identical wire mirror for **all variants** — the
item-level kinds **and** the run/app-level kinds `MixedDrop`/`EngineMissing`/
`WebviewFault`/`BundleDamaged` (§2.13) which `§0.4.3 ErrorKind` also carries — both
are generated, neither is hand-written.)

**Concrete anti-drift enforcement (not just "a check exists") `[DECIDED]`.** The
`ConversionErrorKind` ↔ `§0.4.3 ErrorKind` byte-identical-variant-names guarantee is
enforced by **one of two concrete mechanisms, recommended in order**:
1. **Preferred — make `ErrorKind` a type alias for `ConversionErrorKind`** (`pub type
   ErrorKind = ConversionErrorKind;`) with a single `#[serde(rename_all = "camelCase")]`
   on the owner enum — then there is **one** enum, **no second list to drift**, and the
   wire mirror is the same type. (Viable because both already derive the same traits and
   carry the identical variant set incl. the run/app-level kinds.)
2. **If a distinct type is required** (e.g. the wire enum must omit an internal-only
   variant), keep two enums but add a **compile-time `static_assertions`** check
   (`const_assert_eq!` on the variant counts) **plus** a `#[test]` that asserts every
   `ConversionErrorKind` variant name has a same-spelled `ErrorKind` variant (a
   `strum::VariantNames` round-trip) — so adding a variant to one without mirroring it in
   the other **fails to compile / fails the test**, not merely the §06 codegen-drift diff.

Either way a new variant **cannot silently fail to mirror**; the §06 bindings-drift check
(§0.4.5) remains the third backstop. (§0.4.3 records the wire side of this decision.)

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
| `image_palette` | `→ GIF` (256-colour) | **"Reduced to 256 colours — some colour detail is lost."** |
| `image_downscale` | `→ ICO` (multi-size icon assembly, images.md) | **"Resized to multiple icon sizes — detail may be lost at smaller sizes."** |
| `image_alpha_flatten` | alpha source `→ JPG/BMP` (transparency policy) | **"Transparency isn't supported here and will be filled with a background colour."** |
| `image_animation_flatten` | animated source `→` still target (animation policy) | **"Animated — only the first frame is converted."** |
| `image_svg_raster` | `SVG → raster` (svg entry) | **"Vector image converted to a fixed-size picture ({w}×{h}) — it won't scale up cleanly afterward."** |
| `doc_pdf_reflow` | `DOCX/DOC/ODT/RTF → PDF` **and `MD → PDF`** (documents.md — LO lays Markdown out with reflow/font-substitution like the word-processor sources); **`XLSX/XLS/ODS → PDF` (spreadsheets.md)** — the same office→PDF reflow kind covers spreadsheet→PDF too | **"Layout may shift slightly when converted to PDF."** |
| `doc_pdf_to_text` | `PDF → TXT` | **"Text only — layout, tables and images are dropped."** |
| `doc_html_render` | `HTML → PDF` | **"The result may look different from a web browser."** |
| `doc_to_text` | `* → TXT` from rich sources | **"Text only — formatting and images are dropped."** |
| `doc_simplified` | `* → MD/RTF` from rich sources | **"Some formatting may be simplified."** |
| `sheet_to_delimited` | `XLSX/XLS/ODS → CSV/TSV` (spreadsheets.md) | **"Only one sheet and its values are exported — formatting, formulas and other sheets are dropped."** |
| `xls_legacy_limits` | `* → XLS` (spreadsheets.md) | **"Saved in the old Excel format — rows/columns beyond the legacy limit and newer features are dropped."** |
| `text_encoding_narrowed` | `CSV/TSV → workbook/CSV` with a non-Unicode chosen encoding (spreadsheets.md) | **"Some characters can't be saved in the chosen encoding and would be lost."** |
| `slides_to_pdf_flatten` | `PPTX/PPT/ODP → PDF` (presentations.md) | **"Animations, transitions and embedded media are flattened or dropped, and editing is no longer possible."** |
| `office_roundtrip_approx` | ODF↔MS office round-trip: `ODP → PPTX/PPT`, `PPTX → ODP` (presentations.md); also slide `→ PPTX/PPT` re-layout | **"Some effects and layout may shift when converting between PowerPoint and OpenDocument."** |
| `pptx_to_ppt_legacy` | `PPTX → PPT` (presentations.md) — downgrade to the legacy BIFF8/PowerPoint-97 format | **"Saved in the old PowerPoint format — SmartArt, modern charts, and newer transitions (e.g. Morph) can't be stored and are simplified or dropped."** |
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

- **Predictable only.** A note appears **only** when loss is *predictable* at the
  moment of target choice. For **video**, the precise per-item remux-vs-re-encode
  disposition is **not** known before convert (the full `ffprobe` stream inventory
  is deferred to convert-time, §1.2/§3.5 — running it on every item of a thousands-
  file recursive batch up front is too costly). So the `video_reencode` note is an
  **explicit header-derived best-effort / worst-case** signal computed at target
  choice from the **container pair** (the static matrix flag in video.md) and any
  cheap header hint: if the chosen target pair is **always re-encode** (e.g.
  →WEBM, or a legacy-source container whose inner codecs are known-incompatible) the
  note shows; if the pair is **commonly a remux** but a given item *might* still
  re-encode, the note is phrased as the worst-case *"may be re-encoded"* (honest, not
  a false promise of losslessness). The **precise** per-item disposition is resolved
  only at convert-time (§3.5 `ffprobe`), and the summary (§1.12) reflects what
  actually happened. This keeps §1.2 (header-only detection), §2.9.2 (the note) and
  §0.4.2 (`RunStarted.willReencode`) in agreement: all three are **best-effort
  worst-case before convert, exact after**.
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
  `PathBuf`/`OsString`. ConvertIA **never** lossily converts a path to `String` for
  any *operation* — operations are **`OsStr`-lossless end-to-end** (freeze, identity,
  plan, engine argv, write all use the original `OsString`).
  **No real path bytes cross the WebView wire at all `[DECIDED 2026-07-06 owner
  ruling]`:** the IPC wire and the `app://` events carry only **IDs + core-produced
  display strings** (§0.4.1/§0.4.2 — the real `PathBuf`s live in the core-side
  registries, §0.4.4), and lossy conversion exists **only** in producing those
  display strings — `to_string_lossy()` at the very last step, display-only (the
  §0.6 `display_name`/`*_display` fields). This is forced as well as chosen: JSON
  strings are UTF-8-only, so a serde `Path`/`PathBuf` serialisation cannot represent
  a non-UTF-8 name losslessly — keeping paths off the wire is what keeps this
  guarantee airtight rather than best-effort. **Consequence for a non-UTF-8 name:**
  such a file converts **flawlessly** (every operation runs on the original
  `OsString`, core-side); only its on-screen NAME renders with U+FFFD (the
  replacement character).
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
  decided "bundle everything"), so no engine is fetched at runtime. ConvertIA's Rust
  core makes **no outbound network calls** of any kind for a conversion — there is no
  HTTP client in the conversion path (this is the **T9a** half — the app's own code
  opens no socket). **Bundling alone does NOT prove a bundled engine cannot reach out on
  hostile input** (the **T9b** half — a crafted file driving FFmpeg HLS/DASH/concat,
  pandoc includes, or LibreOffice remote/OLE links): that is closed **structurally** by
  **always-on, cheap-tier argv/build controls** independent of the §2.12 OS sandbox, on
  **both** the SSRF half and the absolute-file LFR half — FFmpeg `-protocol_whitelist
  file,pipe` + network-disabled build (SSRF) **and** concat `-safe 1` (never `-safe 0`) +
  a curated demuxer set without playlist/manifest dereferencing demuxers (absolute-file
  LFR), §3.5.1; pandoc `--sandbox` (§3.5.4); LibreOffice profile-hardening with no remote/
  OLE link auto-update (§3.5.2); **SVG/librsvg — BOTH halves: no remote `<image href>`
  fetch (SSRF) AND the absolute-file LFR half closed by loading the SVG via `rsvg::Loader`
  with NO base URL/`base_file`, so librsvg refuses all local `<image href>`/XInclude
  resolution by construction (no base URL = nothing to resolve against); calls librsvg
  directly since libvips `svgload` has no external-resource toggle; no base-URL confinement
  is used (any base URL is what re-enables the CVE-2023-38633-class surface) (§3.5.5; §6.1.3
  corpus assertion)**.
  The §2.12 wrapper's sandbox profile can **additionally** deny network syscalls and
  restrict the filesystem as defence-in-depth, but it is **not** the load-bearing control
  (it degrades to the cheap tier with no network/FS deny). These are content-fidelity
  *and* offline guarantees.

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

> **Benign vs adversarial scope.** This gate runs the **benign** corpus (it proves
> T9a — ConvertIA's own code opens no socket — and catches an accidental fetch). It does
> **not** by itself prove **T9b** (a bundled engine coerced to reach out by a *crafted*
> input). T9b is closed structurally by the §3.5.1/§3.5.4/§3.5.2 argv/build controls —
> **both** the network/SSRF half (FFmpeg `-protocol_whitelist file,pipe` + network-disabled
> build) **and** the absolute-file LFR half (FFmpeg concat `-safe 1`, never `-safe 0`, +
> the curated demuxer set without playlist/manifest dereferencing demuxers; pandoc
> `--sandbox`; LibreOffice link-update-off) — and verified by the **§6.4.2 adversarial-
> egress case** (a network-trigger input must show **zero egress AND no out-of-input file
> read**), which runs inside this same packet-monitor / egress-deny window. Cite the argv/
> build controls — not "all engines bundled", and not the degradable §2.12.3 OS tier — as
> the T9b evidence.

- **Per-platform packet monitor / egress block — OBSERVE THE ATTEMPT, not just the
  silence (named, §6.7.3 owns the wiring) `[DECIDED]`:** a **bare `-j DROP` / `unshare
  --net` silently swallows the very outbound packet the monitor needs to see** — "zero
  packets observed" then proves nothing (the deny itself hid the attempt). The gate must
  therefore make a blocked-but-**attempted** egress **observable** and fail the release on
  the attempt:
  - **Linux:** instead of bare `-j DROP`, use **`iptables -A OUTPUT … -j LOG`/`NFLOG`** (log
    the attempt) **or** `conntrack`/an `ACCEPT`-to-a-black-hole sink with a sniffer (`tcpdump`
    on a dummy interface) — **or** `strace`/eBPF on `connect()`/`socket()`/`sendto()`; the
    attempt is recorded above the drop. (`unshare --net` with loopback-only still works for
    a *full block*, but pair it with `strace`/eBPF on the connect syscalls so an *attempt*
    is visible, not silently dropped.)
  - **macOS:** a `pf` profile that **logs** matched outbound (`pass log`/`block log` to
    `pflog0` read by `tcpdump -i pflog0`) — or `dtrace` on the connect path — so the
    attempt is captured even while blocked.
  - **Windows:** a Windows Firewall outbound-**block** rule **with logging enabled**
    (the dropped-packets log) — or ETW on the connect/socket path.

  **Any observed outbound *attempt* fails the release** (zero *attempts* is the load-bearing
  proof; "zero packets left the box" alone is not, because the deny would hide them).
- **The §7.2.3 startup `--version` smoke-probe + warm-launch checks are WITHIN this
  gate's scope `[DECIDED]`.** Those probes **spawn third-party engine binaries**, so to
  prove "zero startup network" they run **inside the same packet-monitor / egress-deny
  window** (and, where the §2.12.3 privilege-drop tier is enabled, under network-deny).
  Each is spawned with the §3.5 **minimal env** (no `http_proxy`/`https_proxy`/`*_PROXY`
  vars, `LD_PRELOAD`/`DYLD_*` stripped), so an engine cannot reach out at probe time.
  Net: "zero startup network" is **observably enforced for engine *spawns*** (startup
  smoke-probe + first warm launch), not only for full conversions.

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

### 2.12.3 Hardening the subprocess (defence-in-depth) `[DECIDED — two tiers]`

Beyond the process boundary, ConvertIA drops the decoder's privileges so a
*compromised* (not merely crashing) decoder can do minimal damage. The mechanism is
**per-OS** and is split into two `[DECIDED]` tiers (the cheap tier is a hard v1 floor;
the privilege-drop tier is best-effort, degrading silently — see the callout):

- **All platforms (cheap, v1):** spawn each engine with **(a)** a working directory
  set to the **per-run scratch dir** (§2.6) so relative paths can't wander; **(b)**
  a **minimal environment** (cleared env except what the engine needs — no inherited
  secrets); **(c)** the §2.12.1 process boundary; **(d)** the §1.7 timeout. The
  engine is handed **only** the exact input path and the `tmp` output path (§3.5),
  not a directory it can scan.
- **Linux (recommended v1 if feasible):** wrap the spawn in a **seccomp-bpf** filter
  (e.g. via the `seccompiler`/`extrasafe` crate) denying **exec + unexpected syscalls**
  best-effort, and/or **Landlock** (kernel ≥ 5.13, `landlock` crate) restricting the
  decoder's filesystem to `{input file (ro), tmp dir (rw)}`. **Network deny — use a
  network namespace, NOT seccomp socket-filtering `[DECIDED]`:** seccomp-bpf is **not a
  reliable egress block** — on some ABIs socket calls are multiplexed through
  `socketcall` (so a single allow/deny does not cleanly gate them), `AF_INET` socket
  creation is widely needed by benign libc paths, and seccomp **cannot inspect the
  `sockaddr`** to distinguish loopback from egress. So for network deny the preferred
  mechanism is a **network namespace** (`unshare --net`, loopback-only — the same
  primitive §2.11.4 uses for the egress gate) where the portable build permits it, with
  Landlock(FS) for the filesystem half. seccomp denies exec/unexpected syscalls as
  defence-in-depth, **not** as the egress block. This tier is **defence-in-depth only**
  (§0.10/§2.11.1): the load-bearing offline proof is §3.3.4 (nothing to fetch) + the
  §2.11.4 packet gate, neither of which depends on this OS tier.
- **macOS (recommended v1 if feasible):** run the engine under a **`sandbox-exec`
  profile** / Seatbelt SBPL restricting it to read the input + write the scratch dir,
  deny network and process-exec. (Apple deprecates `sandbox-exec` as a CLI **and
  `sandbox_init` is a private/unsupported API** — not part of the stable platform
  contract — so on an **unsigned, portable** build the Seatbelt route **most often
  degrades to the cheap tier** in practice. This is fine: it is **explicitly accepted**
  because the macOS privilege-drop tier is **not load-bearing** — the T9b network/LFR
  guarantee rests on the always-on argv/build controls (§3.5/§6.1.3) and the offline
  guarantee on §3.3.4 + the §2.11.4 packet gate, neither of which depends on Seatbelt.
  This is exactly why T9b/offline correctly do not depend on this tier.)
- **Windows (recommended v1 if feasible):** spawn in a **restricted token / App
  Container** with a **low-integrity** token, inside a **Job Object** with
  **`JOB_OBJECT_LIMIT`** flags (kill-on-job-close so no orphan survives, memory cap).
  **Network confinement is NOT a Job-Object capability `[DECIDED]`** — `JOB_OBJECT_LIMIT`
  flags govern memory/CPU/process-count/UI, **not sockets**. Network is denied instead by
  **either** an **AppContainer network-isolation profile** (an AppContainer with **no**
  `internetClient`/`internetClientServer`/`privateNetworkClientServer` capability cannot
  open network sockets) **or** a **per-program Windows Firewall/WFP rule**
  (`New-NetFirewallRule -Program <absolute exe path> -Direction Outbound -Action Block`).
  The **Job Object is only the group-kill / resource-cap mechanism** (shared with §1.7),
  **never** the network bound. As with the Linux split (net namespace, not seccomp, is the
  egress block), the network confinement is named to the mechanism that actually provides
  it; the §2.11.4 packet-monitor is the load-bearing offline gate regardless of tier.

> **v1 sandbox depth per OS — `[DECIDED]` (two tiers, owner §2.12):**
> - **Cheap tier = the NON-NEGOTIABLE v1 floor on all three OSes `[DECIDED]`:** the
>   §2.12.1 process boundary + the §1.7 timeout + a **minimal/cleared environment**
>   (no inherited secrets, and **`LD_PRELOAD`/`LD_LIBRARY_PATH` (Linux) / `DYLD_*`
>   (macOS) stripped**) + a **scratch-cwd** working directory + handing the engine only
>   the exact input + `tmp` output paths. This tier needs no elevated rights and never
>   breaks the portable build, so it ships **unconditionally** on Win/macOS/Linux. It is
>   what the SSOT *Security posture* requires.
> - **Privilege-drop tier = `[DECIDED]` best-effort, silent-degrade:** seccomp-bpf /
>   Landlock (Linux), Seatbelt / `sandbox_init` profile (macOS), restricted-token /
>   AppContainer + low-integrity + Job-Object resource caps (Windows) — with network deny
>   via the **AppContainer network-isolation profile or a per-program firewall/WFP rule**
>   (NOT the Job Object, which cannot restrict sockets). It is enabled **where it works
>   WITHOUT install-time elevation and without breaking the portable build**, and
>   **degrades silently to the cheap tier** on any machine/OS-version where it cannot be
>   enabled (kernel too old, profile mechanism unavailable, portable-build constraint).
>   It is **best-effort defence-in-depth, NOT a load-bearing guarantee** — the T9b
>   network/LFR guarantee rests on the always-on argv/build controls (§3.5/§6.1.3), not
>   on this tier (§0.11 T9b). So Phase 3 has a clear floor (cheap tier, mandatory) and a
>   clear best-effort target (privilege-drop tier, where achievable).
> The **only residual** is the *precise per-OS privilege-drop profile contents*
> (which exact syscalls/paths each profile allows) — `[DEFER: tuning]`, a tuning detail,
> not a commitment question; the tier model itself is `[DECIDED]`. Feeds §0.11 and §6.

### 2.12.4 Where detection runs relative to the boundary `[DECIDED]`

Detection (§1.2) is the **first code touching untrusted bytes**. ConvertIA's
detection is **header/magic-byte sniffing only** (a bounded read of the first N
bytes + light structure checks), implemented in **safe Rust** with **no full
decode** — so it is acceptable to run **in-core** (it doesn't invoke a third-party
C/C++ decoder). The moment a full decode is needed (the actual conversion), that runs
in an isolated subprocess. §1.2 states this; §2.12 confirms the boundary, stated
precisely: *no third-party **C/C++** decoder library is linked into or run inside the
Rust core — every full decode runs in a separate subprocess*. The in-core operations on
untrusted bytes are: (a) a **small set of bounded, memory-safe pure-Rust sniffs** — the
text-encoding heuristic, the Rust ZIP central-directory peek, the `.svgz` bounded
inflate (`flate2 rust_backend`/miniz_oxide, ≤64 KiB + ≤100× ratio cap, §1.2 step 2),
**the OLE2/CFB stream-directory read** (legacy `.doc`/`.xls`/`.ppt` detection, §1.2),
and the **bounded XML structural peeks** (`xl/workbook.xml`, ODS `content.xml` — read
with a `quick-xml`/`roxmltree` reader whose **DTD/external-entity resolution is disabled
by construction**, defeating XXE / billion-laughs; G48 asserts this) — which are **not**
full decodes and run no C/C++ decoder; and (b) the **native CSV/TSV
`InProcessNative` conversion** (§3.5.6), which *is* a full in-core untrusted-byte
**transform** (not a mere sniff) but is **acceptable in-core** for the same structural
reason: it is **pure memory-safe Rust** doing a **bounded, streamed** re-encode/re-quote
(no third-party C/C++ decoder, no unbounded buffering — the §1.10 input-size guard bounds
CSV-expansion DoS, §1.7 `InProcessNative` sub-case). The §2.12.4 **absolute is about
third-party C/C++ decoders, not "only sniffs run in-core"** — so the native CSV/TSV path
does not weaken it. Whether the pure-Rust sniffs may stay outside the §2.12 boundary is
now **`[DECIDED]`: yes** — the text-encoding heuristic, the Rust ZIP central-directory
peek, and the `.svgz` bounded inflate (pure safe Rust, no C/C++ decoder, ≤64 KiB + ≤100×
caps) **stay in-core** (memory-safe, bounded, not a full decode), so the §2.12.4 absolute
is satisfied (it forbids third-party **C/C++** decoders in-core, which none of these are).
(Moved off `[OPEN]` in the consolidation pass — README resolved log.) The absolute
as worded above is **not** weakened by any of these because none invokes a third-party
C/C++ decoder. This is true for **all** engines including the
image core: image decode/encode runs in a **separate image-worker process**
`[DECIDED]` (§0.7/§3.5.5 — the README/§3.5.5 in-process-vs-worker `[OPEN]` is resolved
to the worker), so a memory-corruption exploit in libvips/libheif/libde265/librsvg/a
TIFF loader executes inside that throwaway worker's address space, **not** ConvertIA's
core — the §2.12.1 process boundary contains it exactly as for FFmpeg/LibreOffice and
T1 (§0.11) is uniformly subprocess-isolated. (This also reinforces §3.6: copyleft
engines are aggregated as separate binaries; the image-worker links libvips/LGPL
internally, which is aggregation, never a link into the MIT core.)

> **Note — the §2.13 `catch_unwind` boundary is NOT a containment mechanism for
> hostile native code.** It catches *Rust* panics in ConvertIA's own orchestration
> code at the item boundary; it does **not** contain arbitrary-code-execution or
> memory corruption inside a native decoder. The only thing that contains a
> compromised decoder is the **OS process boundary** above (plus the §2.12.3
> privilege-drop tier). Do not cite `catch_unwind` as a security boundary against
> untrusted decoder input.

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
- **Intake/detection panic boundary (C1 `drain_intake`) `[DECIDED]`.** The §2.13.2
  per-item boundary above covers the *convert* loop, but the
  **§1.1 walk + §1.2 detection** (the magic sniff, the `.svgz` bounded inflate, chardetng,
  the Rust ZIP central-directory / OLE peeks — the **first** code to touch **untrusted
  bytes**) runs **inside the C1 `drain_intake` command handler** (every intake source
  funnels into it via the §7.8.1 `PendingIntake` buffer `[DECIDED 2026-07-06]`; C2a
  only opens the picker and buffers), *before* any conversion item exists.
  That code is therefore **also wrapped in `catch_unwind`**, at two granularities:
  - **Per-path:** the detection of **one** path runs inside `catch_unwind`; a panic
    decoding one file's header becomes that path's `DetectionOutcome::Uncertain` (it does
    **not** abort the whole walk) — the walk continues to the next path.
  - **Whole-walk:** the C1 handler's outer body is itself wrapped so a panic that
    escapes the per-path boundary (e.g. in the recursion/dedup bookkeeping) is converted
    to a **calm `IpcError`** returned from the command (a `CollectedSet`-level failure the
    §5.2 *Collecting* state renders as "couldn't read these files", **never a blank
    window**), not an unwind across the Tauri command boundary.
  This makes the §0.4.0 "no command ever panics across the boundary" claim true for the
  **intake** path, not only the convert path. (Tauri v2 command futures do not themselves
  guarantee a user-visible result on panic — the explicit `catch_unwind` here is the
  mechanism, mirroring §2.13.2.)

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

  > **Presentation channel by WebView health `[DECIDED]` (P2.109).** *Which* surface a
  > **startup** fault renders on is fixed by whether the WebView itself is alive.
  > **`EngineMissing` / `BundleDamaged`** (a §7.2.1 readiness fault, steps 3–5) leave the
  > WebView healthy → they present over the §0.4.2 `app://fault` event on the §5.8 WebView
  > screen, replayed through a **`PendingFault`** buffer for the first-frame race (the
  > `app://fault` emit + buffer body lands with the P4 readiness verifier bodies).
  > **`WebviewFault`** *at startup* (§7.2.1 step 6 — `get_webview_window("main")` is `None`:
  > a missing/old WKWebView / WebKitGTK, §0.3.1) makes an `app://fault`→WebView emit
  > impossible, so it renders on a **native surface** (not the WebView; the concrete native
  > mechanism is a P4 decision). Both route through the mechanism-independent
  > `present_startup_fault` entry (§7.2.1), which records to the local log (§7.5) now; the
  > two presentation bodies are P4. **P2.109 builds the `WebviewFault` detection + routing
  > seam.** (A `WebviewFault` *mid-run* — the WebView was alive and lost the IPC channel —
  > is the separate §5.8 disconnect bullet below, which may still render in the surviving
  > WebView.)
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

**Scratch-residue confidentiality posture `[DECIDED]` (accepted residual, not a silent gap).**
For the macOS TCC staging (§3.5.0) and the cross-volume copy (§2.14.3) a **copy of the
source** may transit kind-2 scratch, and after an *ungraceful* end (power loss, SIGKILL) it
may persist as **plaintext in scratch** until the next-run §2.6.3/§2.7.2 orphan reclaim.
ConvertIA performs **no secure/zeroing wipe** of scratch (a best-effort `unlink` only) — for
a **local / private / single-user** offline product this is an **explicitly-accepted residual**,
not an undisclosed gap, consistent with the SSOT *local-private* posture; secure erase of
copied source bytes is **out of scope**. (The §2.6 cleanup-residue *honesty* — telling the user
a temp file may remain and where — is a separate, in-scope promise; this note is only about the
absence of cryptographic wipe.)

### 2.14.1 Same-volume rule: scratch goes next to the *final*, not next to the *source* `[DECIDED]`

The atomic-publish (§2.1.2) is a `rename(tmp → final)`, which is only atomic
**within one volume**. Therefore the **invariant**:

> **`tmp` is always created on the same volume as `final`** (the *destination*), not
> necessarily the same volume as the source.

Concretely, `crate::run` picks the publish-temp path **inside the destination
directory itself** (same volume by construction). The chosen form is a
**uniquely-named dotfile *sibling* of `final`**, not a subdir, and it **embeds the
owning `InstanceId` as well as the `RunId`** so its authoritative lock is directly
addressable from the filename alone:
`…/<dest_dir>/.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`. A bare **file**
(rather than a `.convertia-run-<RunId>/` subdir) is deliberate: it lets the §2.6.3
startup/next-write sweep **opportunistically remove a sibling stale
`.convertia-*.part`** (whose embedded `InstanceId`+`RunId` lets the sweep find the
**exact** owning lock at `convertia/scratch/<InstanceId>.*/run-<RunId>/.lock` without
scanning every instance subdir — and a lock that is **held** ⇒ live ⇒ skip; **free,
stale, OR entirely absent** ⇒ dead ⇒ reclaimable, **safe ONLY because of the §2.6.3
lock-before-part ordering invariant** — `run-<RunId>/.lock` is created and held **before**
the run writes its first `.part`, so a live `.part` can never coexist with an absent lock)
without having to discover and tear
down a directory, and it keeps the no-placeholder publish (§2.1.2) a single same-dir
rename.
This is what makes the §2.1 publish a true intra-volume atomic rename in the common
beside-source case (dest dir = source dir = one volume) **and** in the divert case
(dest dir = Downloads = system volume; publish temp also on the system volume).

- *Recommended publish-temp placement:* a **dotfile sibling** in the destination
  directory, removed by §2.6 on run end (and opportunistically by a later write,
  §2.6.3). This avoids any cross-volume move for the *publish*. If the destination
  directory itself is not writable, §2.7 has **already diverted** the destination to
  a writable one — so by the time §2.14 places the publish temp, the destination is
  known-writable (§2.7.2 probe).
- *Alternative considered & rejected for the publish:* a single global app scratch
  dir (e.g. under the OS temp) for *all* runs. Rejected as the *publish* temp because
  it is frequently on a **different volume** than a beside-source destination,
  forcing the cross-volume fallback (2.14.3) on the **common** path. The global temp
  is fine for **transient engine working files** that are *not* the publish artifact
  (see 2.14.2).
- **Mode bits `[DECIDED]` (so the §0.11 temp-ownership gate has a concrete target):**
  on POSIX the **per-run scratch root** is created `0o700` (owner-only) and the
  `.part` **publish-temp** `0o600` — a per-run scratch must never be world-readable in
  a shared `/tmp`/`$TMPDIR`, since transient engine working files may briefly hold
  converted bytes. On Windows the equivalent is the default per-user ACL (the scratch
  lives under the user profile / `app_local_data_dir()`; no explicit broadening). A
  unit/property gate asserts these modes (build-gates G15/G31).

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

**macOS TCC source-staging copy is a THIRD kind-2 contributor (macOS-only) `[DECIDED]`.**
On macOS the Rust core **copies every beside-source input into kind-2 scratch before
spawning** any engine (§3.5.0 / §7.2.6 TCC staging — the engine never first-touches a
protected path). That staged copy is **input-sized, per item, on the scratch/system
volume** — a real kind-2 footprint the §2.14.4 / §1.10 free-space model must count. On
**Windows/Linux this term is 0** (no TCC, no staging). So on macOS the kind-2 estimate is
`LibreOffice/FFmpeg working space + staged input sizes for the in-flight set`; a macOS
batch of large videos/PDFs can exhaust the **system/scratch volume** even though the
*output* fits the destination — which is exactly why it must be in the preflight.
**PEAK-CONCURRENT, not whole-batch Σ `[DECIDED]`:** staged source copies are reclaimed
**per item** as each engine finishes, so at most §0.9 **concurrency-degree** of them coexist
— the §1.10 preflight must bound this term to the **peak concurrent footprint** (~`degree ×
largest in-flight staged inputs`), **not** the sum of every item's staged-input size across
the whole batch (which would over-count and falsely trip `OutOfDisk`).

**Staged-copy lifecycle — created AFTER the run-lock, reclaimed unconditionally
`[DECIDED]`.** The macOS staged **source** copy is a kind-2 file under the **per-run
scratch root** `convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`, and it is created **after the
`run-<RunId>/.lock` is acquired** — the same **lock-before-part ordering invariant**
(§2.6.3 / §2.14.1) that covers `.part` files therefore covers the staged source copy too,
so the §2.6.3 startup sweep reclaims it on the next launch after a crash (**absent lock ⇒
dead ⇒ reclaimable**). It is also reclaimed by the §2.6.2 **run-scope cleanup
unconditionally** on every normal/cancel/error exit (it lives inside the run dir that
`cleanup_run` removes). A cancel between staging and engine-spawn therefore leaves no
stranded source copy; a crash there is reclaimed at next launch. **§6.4.2 property-test
case (added):** *kill the app between the staged source copy and the engine spawn; on next
launch assert the staged copy (and its `run-<RunId>/` dir) is reclaimed by the startup
sweep* — enumerated alongside the kind-1 `.part` crash cases so the staged SOURCE copy is
not an untested residue path.

**Linux AppImage topology (no special handling needed) `[DECIDED]`.** On an AppImage,
the app itself runs from a **read-only squashfs mount** — but the kind-2 scratch root
resolves to **`app_local_data_dir()`** (under the user's writable home, e.g.
`~/.local/share/…`), which is on the **system volume**, not the squashfs mount, and
is a valid kind-2 scratch root. The kind-1 **publish temp** still lives on the
*destination* volume per §2.14.1; when the destination is a different volume (a USB
stick — the expected common case for a portable Linux tool), the §2.14.3 cross-volume
fallback activates exactly as designed. No AppImage-specific code path is needed
beyond the existing kind-1/kind-2 split. **(If that USB stick is FAT32/exFAT — the
typical default format for a portable stick — the destination has no Unix create-only/
atomic-publish primitive at all, so it is diverted at §2.7.2 to the hardlink-capable
system disk rather than written beside-source; see §2.7.2 No-atomic-publish-primitive
test and §2.1.2 third fallback.)**

### 2.14.3 Cross-volume fallback (only when same-volume can't be guaranteed) `[DECIDED]`

In the rare case where the publish temp truly cannot be co-located with `final` on
one volume (e.g. a destination dir that is writable but on a filesystem where
creating a sibling scratch dir is disallowed, or a quirky network mount), the
**fallback preserves atomicity *within the destination volume*** by doing the
move-equivalent **inside** that volume:

1. Write `tmp` in the **best same-volume location obtainable** for `final` (the
   destination dir as a sibling dotfile; if a sibling can't be created there, the
   destination dir's own parent on the same volume).
2. If, despite this, the only available scratch is on **another** volume, **the engine
   is told to write its output to that other-volume scratch** (this is the pre-engine
   temp-PLACEMENT decision the §0.6 invariant-5 note refers to — made before the engine
   runs, *not* a stored `OutputPlan` field), and the publish then performs a
   **copy + fsync + exclusive-publish-within-destination-volume**. **The intermediate
   cross-volume temp has a named, swept home `[DECIDED]`:** that "other-volume" temp is
   **NOT** an anonymous `tempfile` in an arbitrary `$TMPDIR` — it lives under the **per-run
   central scratch root** (`convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`, the kind-2 root
   covered by the run-lock and swept by §2.6.3 step 1), **or**, if it must sit elsewhere on
   that volume, it carries the **same `InstanceId`+`RunId` `.convertia-<InstanceId>-<RunId>-
   <jobId>-<rand>.part` naming** as a kind-1 publish temp (so the §2.6.3 per-file
   opportunistic sweep resolves its owning lock cross-instance). Either way the
   **lock-before-part ordering invariant** (§2.6.3 / §2.14.1) covers it — the
   `run-<RunId>/.lock` is held before this temp is written, so **absent lock ⇒ dead ⇒
   reclaimable** still holds and a crash mid-fallback cannot strand a temp that escapes
   *run-end* cleanup (the §2.6.2 recorded-`final_dir` set includes this intermediate's dir).
   **Honest post-crash limitation `[DECIDED]`:** when the intermediate sits **on a volume
   OTHER than the central-scratch-root volume** (the "elsewhere on that volume" case above),
   the **§2.6.3 startup sweep does NOT enumerate it** — the central-scratch startup sweep
   only walks the central-scratch-root volume (exactly the same limitation §2.6.3 already
   states for a destination-resident `*.part` after a crash). So after a **true crash** an
   off-central-volume cross-volume intermediate is reclaimed only **opportunistically** (a
   later write into that dir, or a manual delete), not by the startup sweep — within the same
   best-effort tolerance as a crashed destination-resident `*.part` (§2.6.3). Then:
   - **re-check destination-volume free space for the intermediate `[DECIDED]`:** the
     copy-into-dest step makes the output's bytes exist a **second time on `final`'s volume**
     (the intermediate copy) — coexisting with the publish temp — so this path's **peak
     destination-volume footprint is ~2× output**. The §1.10 / §2.14.4 preflight models
     `est_output`+publish-temp and `est_scratch`, but **NOT** this cross-volume intermediate,
     so on a near-full destination volume the copy can `ENOSPC` despite preflight passing.
     **Before the copy**, re-check `final`-volume free space against the intermediate's size
     (≈ output) and **fail the item clearly with `OutOfDisk` (§2.8)** if it won't fit —
     mirroring §2.7.2's late-divert "never assume it fits" rule. (Same posture: preflight is
     a best-effort gate, the at-use re-check is the bound.)
   - copy the cross-volume temp into a **new** temp **on `final`'s volume**,
   - `sync_all()` it (durable),
   - then publish that same-volume temp → `final` with the **no-placeholder
     exclusive-rename** (§2.1.2: Linux `renameat2(RENAME_NOREPLACE)` / macOS
     `renameatx_np(RENAME_EXCL)` / common `link`+`unlink` fallback, Windows
     `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`) — intra-volume and exclusive,
     create-only, never a 0-byte placeholder. **The cross-volume copy happens EXACTLY ONCE
     `[DECIDED]`:** if the publish hits a name collision, the §2.2 numbering retry
     **re-renames the SAME already-copied same-volume intermediate** to the next variant —
     it does **NOT** re-copy across the volume per attempt (the expensive cross-volume copy is
     done once; only the cheap intra-volume exclusive-rename loops). **(On Unix this final publish never
     targets a FAT/exFAT-class `final`: such destinations are diverted up front at §2.7.2
     to a hardlink-capable system-disk target, so the no-replace-or-`link` primitive is
     always available where this step runs; on Windows `MoveFileExW` works on FAT/exFAT
     directly.)**
   - `fsync` the destination directory (Unix) for durability.
   This is exactly the documented `EXDEV` remedy (the tempfile-crate guidance:
   *cannot persist across filesystems → copy into the destination volume, then
   rename*). The cross-volume step is a **copy**, never a cross-volume `rename`
   (which would fail `EXDEV`); the **only** rename is intra-volume and exclusive.
3. The extra copy is removed by §2.6. The user-visible result is identical: `final`
   appears atomically or not at all; a crash leaves only discardable temps.

`fs_guard::atomic_publish(tmp, final)` encapsulates all of this: it tries the
direct intra-volume no-placeholder publish (§2.1.2) first, and only on `EXDEV`
(Unix) / cross-device failure (Windows) falls back to copy-into-dest-volume-then-
exclusive-publish. Callers (§2.1) never see the distinction.

### 2.14.4 Space accounting ties to §1.10 `[DECIDED]`

The scratch model means a conversion transiently needs **destination-volume free
space ≈ output size** (publish temp) **plus** any kind-2 working space — and the two
**may live on different physical volumes**. §1.10 (resource pre-flight, `[DEFER: corpus]`
budget digits) owns the up-front estimate and the "doomed for disk" fast-fail; §2.14 **supplies** the
model it estimates against. The free-space check is therefore **per PHYSICAL volume**, and
the footprint is split by where each byte actually lands (§2.14.2):
- **`est_output_bytes` + the kind-1 publish temp (`*.part`)** land on **each item's
  `final_dir` volume** (the destination volume, §2.14.1) — beside-source or divert.
- **`est_scratch_bytes` (kind-2 engine working files — the LibreOffice per-run profile,
  FFmpeg two-pass/internal temp, **plus on macOS the Σ of staged input sizes**, §2.14.2)**
  land on the **system / scratch volume** that
  `app_local_data_dir()`/`temp_dir()` resolves to (§2.14.2), which is **NOT** necessarily
  the destination volume (e.g. a beside-source-on-USB job: output → USB, kind-2 → internal
  disk). **macOS TCC staging term `[DECIDED]`:** on macOS `est_scratch_bytes` **includes
  the sum of staged input sizes** (the §3.5.0/§7.2.6 source-into-scratch copy, input-sized
  per in-flight item) for the system/scratch volume; on **Windows/Linux this term is 0**
  (no TCC staging). So a macOS batch of large inputs is checked against the scratch volume
  for the staging copies, not only the destination volume for the outputs.

So §1.10 **groups the footprint by physical volume across BOTH categories** and requires
headroom on **every** volume the batch touches independently — the destination volume(s)
**and** the system/scratch volume. A 5 GB share destined for a 1 GB stick must fail up
front (destination volume) even when the internal disk has ample room; equally, a heavy
office/video batch whose kind-2 scratch would exhaust the **system volume** must fail up
front even when each destination volume passes. Because the §2.7 beside-source default and
per-location divert can spread one batch across **several destination volumes** (each item
lands on its own `final_dir`'s volume, §2.14.1), there may be 2+ destination volumes plus
the one scratch volume to verify. The to-GIF guardrail (cross-category.md) and video
re-encode estimates feed the same §1.10 per-physical-volume check.

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
