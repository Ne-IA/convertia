# 07 — App Shell (ConvertIA as a running desktop app)

> Everything *around* the conversion pipeline: how the app starts, lives, stores
> (or deliberately doesn't store) state, logs, and updates. Origin: SSOT
> *Portable, no installation*, *Local/private/offline*, *Fail clearly*. This file
> exists because the pipeline (01) and guarantees (02) reference an app/instance
> model that must be defined somewhere.

## 7.1 Instance & run identity `[OPEN]`
- Single- vs multi-instance policy; a **run id** and **instance id** model that
  02.6 (per-run/instance temp ownership) and 02.1 (crash recovery / "cleanup on
  next run") depend on. The SSOT no-clobber text explicitly references "a second
  app instance" / "another instance's in-progress file" — so this is load-bearing,
  not optional. _(decide & expand)_

## 7.2 Startup sequence & first launch (technical)
- Sidecar/engine presence verification; executable-permission setup on extraction
  (portable build); first temp-dir creation; macOS TCC file-access prompts that
  the beside-source default can trigger; what happens if a bundled engine is
  missing/corrupt at startup (→ app-level fault, 02.13). Distinct from the UI
  empty-state (05.2). _(expand)_

## 7.3 Window & app lifecycle
- Window create/restore/close; quit-while-converting (confirm? finish? cancel?);
  in-flight queue on close; tray/background? (likely no). _(expand)_

## 7.4 Persistence & app state `[OPEN]`
- Decide what (if anything) persists between launches: last-used output
  destination, theme, window size/position, future opt-in update flag — vs **"v1
  persists nothing"** as an explicit decision. Where it would live per-OS
  (AppData / Application Support / XDG) vs portable-sidecar, reconciled with SSOT
  *portable / no system pollution*. **Explicit negative:** no history / no
  recent-files (SSOT forbids accounts, parks presets) — session-only summary.
  _(decide & expand)_

## 7.5 Logging & diagnostics (local-only, no telemetry)
- Whether a local log exists; location; rotation/retention; **redaction stance**
  (a log capturing file paths/contents would dent the 02.11 privacy invariant —
  reconcile). Feeds the SECURITY/bug-report path. Dev-facing half (verbose mode,
  echo of exact engine command line, reproducing a one-platform corpus failure)
  makes the 06.5 reliability gate operable. _(decide & expand)_

## 7.6 Update posture (no auto-updater) `[DECIDED: no phone-home]`
- The Tauri stack ships an updater that must be **explicitly disabled/absent**
  (concrete spec item); in-app pointer to the canonical GitHub Releases page is
  **user-initiated only**; where the current version is shown (About, 05.9). Any
  future check would be opt-in/disclosed (SSOT). _(expand)_

## 7.7 OS shell-out (open-folder / open-file / open project page) `[OPEN]`
- **Single owner** of the concrete Tauri opener operations behind the DoD
  core-UX gate (one-click open-folder/file) and the only permitted network
  (user-initiated open-project-page). On the §0.10 allowlist (opener scope).
  Per-OS **reveal-in-folder** behaviour; **open-file safety** — launching an
  external app on a fresh, possibly-untrusted artifact is security-relevant
  (§0.11). §2.7 fixes *which* path; this owns *how* the shell-out works. UI entry
  via §5.3 OpenActions. _(expand)_

## 7.8 OS intake & integration posture (Open-with / launch args; explicit negatives)
- Whether paths arrive as **launch args / OS open-file events** (macOS open-doc,
  Windows `argv`, Linux `%F`) into §1.1 intake, and how that interacts with
  single-vs-multi-instance (§7.1) and the freeze point. **Explicit negatives**
  (mirroring §7.4): ConvertIA registers **no file associations**; **no drag-out,
  no clipboard export** in v1 (SSOT parks drag-out); output OS-integration is
  limited to §7.7 open-folder / open-file. _(expand)_
