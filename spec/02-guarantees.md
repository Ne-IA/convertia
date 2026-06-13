# 02 — Guarantees (implementation of the SSOT hard promises)

> Each load-bearing SSOT guarantee gets a concrete technical implementation here.
> Origin: SSOT *Never harm the original*, *Fail clearly*, *Local/private/offline*,
> *Security posture*. The SSOT states the promise; this file states the mechanism.

## 2.1 No-clobber & atomic write
- Exclusive create (create-new-or-fail) on the **resolved** path; write-to-temp
  then atomic rename; crash/power-loss safety; never a truncated visible file.
  _(expand)_

## 2.2 Output naming contract
- Base name kept + target extension; `(1)`,`(2)` numbering before the extension;
  never hashed/`_converted`; path-limit → fail clearly (no truncation). _(expand)_

## 2.3 Resolved-identity & link safety
- Resolve symlink/alias/junction/hardlink; never write to/through/as a target
  resolving onto a frozen source; divert to fallback; de-dup the frozen set by
  resolved identity. _(expand)_

## 2.4 Frozen source set & no self-feeding
- Snapshot at drop/selection; files appearing after the freeze are never
  ingested; outputs in a source folder don't expand/restart the batch. _(expand)_

## 2.5 Re-run / equivalent-output detection
- Equivalence = same resolved source + target + effective settings; one
  batch-level prompt (skip default / fresh copy); **best-effort**, safe fallback
  to silent numbering when undeterminable. _(expand)_

## 2.6 Cleanup, temp ownership & free-space restoration
- Per-run/instance temp ownership; remove partials on failure/cancel/out-of-disk;
  startup cleanup that never touches another instance's temp; cleanup-failure →
  never reported as clean success. _(expand)_

## 2.7 Output destination & per-location fallback
- Beside-each-source default; chosen destination re-creates relative subtree;
  per-location divert for unwritable/ephemeral locations; flatten handling +
  summary mapping; "open folder" → common root. Guarantees hold on divert path.
  _(expand)_

## 2.8 Error taxonomy & fail-clearly
- Enumerated failure kinds (corrupt, empty/0-byte, unrecognised, unsupported,
  unreadable/gone-mid-batch, too-big, out-of-disk, engine-crash/hang, path-limit);
  plain-language messages; batch-continues; summary. _(expand)_
- **Message catalog (home):** each failure kind → its exact plain-language
  English string lives here as a table, so strings aren't scattered (SSOT *Fail
  clearly*). **02 owns conversion-outcome strings** (failure §2.8 + lossy §2.9);
  UI-chrome strings (empty-state, confirm-gate, buttons, About) live in §5 and
  share the same future-localization boundary. §5.7 surfaces them. _(fill table)_

## 2.9 Lossy disclosure
- Which (source,target) pairs are predictably lossy; passive inline note at
  target choice; not blocking, not per-conversion nag. _(expand)_
- **Message catalog (home):** each predictably-lossy pair → its exact note string
  lives here as a table (cross-ref the 04 per-format lossy flags). _(fill table)_

## 2.10 Filenames & i18n (content + names)
- Unicode/emoji/long-path filename handling; content fidelity (CJK/RTL/encodings,
  CSV delimiters). _(expand)_

## 2.11 Privacy & offline invariants
- No network at all (all bundled); no telemetry/accounts/update-phone-home; the
  cloud-sync caveat; observable "no network" property. _(expand)_

## 2.12 Security / decoder isolation
- Untrusted-input decoders run isolated/contained; a decoder crash/hang fails one
  item without wedging the app or breaking no-harm; isolation mechanism choice.
  **Owner of the per-platform decoder-isolation mechanism** — §0.3 (process
  model), §1.7 (invocation) and §3.5 (sidecar args) reference and route through
  this. Pairs with §0.10 (the WebView/CSP half of security). _(expand)_

## 2.13 App-level fault model (vs per-item) & the app-wide "no stack traces" contract
- Fault classes: **item-level** (§2.8) vs **run-level** vs **app-level** (Rust
  core panic, WebView fails to load, engine binary missing/corrupt at startup,
  damaged bundle, no disk at all). Worker-thread **panic boundary**
  (catch_unwind / isolate-and-report so a panic surfaces as a clean per-item
  failure, not a poisoned pool). Engine-`stderr` capture-and-classify feeds §2.8.
  How an unexpected internal error is shown to a non-technical user **without a
  stack trace** (SSOT *Fail clearly*); startup faults link to §7.2. _(expand)_

## 2.14 Temp / scratch space & cross-volume atomic strategy
- **Single owner** of where scratch lives and on which volume. Atomic rename
  (§2.1) requires temp + final on the **same filesystem**, yet beside-source +
  per-location divert (§2.7) can put source / scratch / final on **three**
  volumes (USB source → Downloads). Define: the scratch-location policy (per-run,
  §2.6 ownership), how the final move stays atomic, and the **cross-volume
  fallback** (copy → fsync → atomic rename *within* the destination volume).
  §2.1 / §2.6 / §1.10 / §3.5 / §7.2 reference this instead of each implying its
  own temp model. _(expand)_
