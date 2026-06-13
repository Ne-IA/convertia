# 07 — App Shell (ConvertIA as a running desktop app)

> Everything *around* the conversion pipeline: how the app starts, lives, stores
> (or deliberately doesn't store) state, logs, and updates. Origin: SSOT
> *Portable, no installation*, *Local/private/offline*, *Fail clearly*, *Never
> harm the original*. This file exists because the pipeline (§1) and guarantees
> (§2) reference an app/instance model that must be defined somewhere.
>
> **Read together with [00-architecture](00-architecture.md)** — the process
> model there depends on the instance/run-identity model defined here (§7.1).
>
> **Ownership.** This file OWNS: instance & run identity (§7.1), startup &
> first-launch technicals (§7.2), window/app lifecycle (§7.3), persistence posture
> (§7.4), local logging/diagnostics (§7.5), update posture (§7.6), the concrete
> OS shell-out operations (§7.7), and the OS intake/integration posture incl.
> explicit negatives (§7.8). It REFERENCES (does not restate): the IPC contract
> (§0.4), the capabilities/CSP allowlist (§0.10), the concurrency degree (§0.9),
> the intake/freeze flow (§1.1/§2.4), temp ownership & cross-volume atomicity
> (§2.6/§2.14), the app-fault model & "no stack traces" (§2.13), privacy
> invariants (§2.11), the sidecar arg construction (§3.5), and the UI states/
> About/OpenActions (§5.2/§5.3/§5.9).
>
> Decision tags: `[DECIDED]` fixed here/by SSOT · `[OPEN]` owner-level call (feeds
> the README open-questions log) · `[REC]` an `[OPEN]` resolved here with a
> recommended default · `[DEFER]` settled during implementation.

---

## 7.1 Instance & run identity `[REC]`

> SSOT origin: *Never harm the original* ("a second app instance",
> "another instance's in-progress file", "outputs landing in a source folder do
> not expand or restart the batch"). Load-bearing for §2.6 (per-run/instance temp
> ownership), §2.4 (frozen source set), §2.14 (scratch), §0.9 (subprocess pool).

### 7.1.1 Single-instance policy `[REC: single-instance, hand-off]`

**Recommendation: ConvertIA runs as a single GUI instance per OS user session,**
using the official **`tauri-plugin-single-instance`** (v2). Rationale:

- The SSOT no-clobber guarantee is "absolute" and evaluated on the *resolved real
  file* with an exclusive create-new-or-fail final write (§2.1). That guarantee
  is correct even with two independent processes — but a single instance makes
  per-run temp ownership (§2.6) and the "cleanup on next run never touches another
  instance's in-progress file" rule **dramatically simpler to reason about and
  test**, and avoids two WebView processes + two LibreOffice headless profiles
  (LibreOffice headless is **not safely parallel under one user profile** — §0.9).
- A second launch (double-clicking the portable binary again, or an OS
  "Open with" hand-off, §7.8) must **not** spin up a competing converter.

**Mechanism (Rust, `src-tauri/`):**

```rust
// registered FIRST in the Builder so it wins before any window is created
.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
    // Running in the EXISTING (primary) instance. The second process has
    // already exited. Re-focus the window and forward any launch-time paths.
    let _ = app.get_webview_window("main").map(|w| { let _ = w.set_focus(); });
    forward_launch_intake(app, &argv, &cwd); // → §7.8 → §1.1 frozen-set intake
}))
```

- On **Windows/Linux** this is the only way a second launch (and any OS-passed
  file argv, §7.8) reaches the primary instance — without it the OS spawns a new
  process. On **macOS** the OS already routes a second open to the running app via
  the open-documents AppleEvent (§7.8); single-instance is kept for parity and to
  cover direct binary re-execution of the portable build.
- **Multi-user / fast-user-switching:** the lock is **per OS user**, not
  machine-global (the plugin's default lock scope), so two different logged-in
  users may each run their own instance — acceptable because their temp/scratch
  (§2.14) and output locations are user-scoped anyway.

`[OPEN→README]` *Remaining owner call:* whether the second-launch hand-off, when
the primary instance is **mid-conversion**, (a) silently queues the new paths as
a fresh idle-state drop once the current batch ends, or (b) is refused with a
calm "ConvertIA is busy — finish or cancel the current batch first" note. **`[REC]`
option (b)** for v1: it keeps the freeze point (§2.4) and the one-batch-at-a-time
model (§1.3) unambiguous and avoids a hidden queue the user can't see. UI surface
in §5.2.

### 7.1.2 `InstanceId` and `RunId` model `[DECIDED]`

These two ids are the entities §0.6 lists as "defined in §7.1". Both are
**process-local, never persisted, never networked** (§2.11).

| Id | Type | Scope / lifetime | Derivation | Purpose |
|----|------|------------------|------------|---------|
| `InstanceId` | `Uuid` (v4) — opaque 128-bit | One running process, created once in `setup` | Random at launch | Names the per-instance scratch root (§2.14) and stamps temp artifacts so startup cleanup (§2.6) can tell *this* instance's residue from a *different* instance's still-running temp |
| `RunId` | `Uuid` (v4) | One "drop → … → summary" cycle (one `Batch`); a new drop after a summary starts a new `RunId` | Random when the frozen source set is created (§2.4) | Owns the per-run temp subdir; cancellation/cleanup (§2.6), progress events (§0.4) and the end-of-batch summary (§1.12) are all keyed by it |

Pseudo-types (mirrored to TS via the §0.4.5 mechanism — not re-decided here):

```rust
pub struct InstanceId(pub Uuid);   // app-managed singleton via app.manage(...)
pub struct RunId(pub Uuid);        // field of `Batch`/`RunResult` (§0.6)
```

**Scratch-root naming (the load-bearing detail §2.6/§2.14 depend on):** the
per-instance scratch root is named with **both** the `InstanceId` **and the OS
PID**, e.g. `…/convertia/scratch/<InstanceId>.<pid>/`. The combination lets
startup cleanup (§7.2.5, mechanism owned by §2.6) safely reclaim an *orphaned*
scratch root (process gone, PID not alive / not us) while never deleting a
**concurrent same-user instance's** live root. Per-run subdirs live under it as
`…/<InstanceId>.<pid>/run-<RunId>/`. (Exact path policy is owned by §2.14; this
section only fixes the *identity* embedded in it.)

---

## 7.2 Startup sequence & first launch (technical) `[DECIDED + REC]`

> SSOT origin: *Portable, no installation*; *v1 Definition of Done* (offline
> floor, observable no-network). Distinct from the UI empty-state (§5.2): this is
> what the **core** does before the first frame; §5.2 is what the user then sees.

### 7.2.1 Ordered startup sequence

1. **Single-instance guard** (§7.1.1) — registered first; a second launch hands
   off and this process exits before doing anything else.
2. **Establish `InstanceId`** (§7.1.2) and resolve base paths via the Tauri path
   API (`app.path()`): config dir, local-data/scratch dir (§2.14), log dir
   (§7.5). No directory is *created* yet.
3. **Engine presence + integrity verification** (§7.2.3) — the bundled sidecars
   must exist and be runnable; a failure here is an **app-level fault** (§2.13),
   not a per-item failure.
4. **Executable-permission setup** on the engine binaries for the portable build
   (§7.2.4).
5. **Scratch + log dir creation** with the per-instance root (§7.1.2). Reclaim
   orphaned scratch roots (§7.2.5, owned by §2.6).
6. **WebView window create** and frontend load (the WebView runtime floor is
   §0.3.1; a missing/old WebView is itself a §7.2/§2.13 startup fault).
7. **Process launch-time intake** (§7.8): if the app was opened *with* file paths
   (OS open-doc / argv), feed them through §1.1 once the window is ready.
8. Hand to UI empty/idle state (§5.2).

Steps 3–5 run in the Rust core during `setup`/just after; the window is only shown
once they succeed, so a hard fault is shown as a clean fault dialog (§2.13), never
a half-broken UI.

### 7.2.2 Offline assertion at startup `[DECIDED]`

ConvertIA performs **no** network call at startup (or ever, as a result of its own
behaviour — §2.11): no update check (§7.6), no license/telemetry beacon, no font/
asset fetch (all assets are bundled; CSP forbids remote origins — §0.10). This is
an *observable* property and a §6.5/§2.11 release gate; §7 asserts only that the
shell adds **zero** startup network activity.

### 7.2.3 Sidecar/engine presence & integrity verification `[REC]`

Engines are **bundled** as separate invoked binaries (§3.3/§3.6) — never fetched
(SSOT *Local/private/offline*). At startup the core verifies the engine set is
present and usable:

- **Presence:** resolve each expected sidecar/resource path (under the Tauri
  resource dir / sidecar location, §0.7) and confirm the file exists. The
  authoritative *list* of expected engines per platform is owned by §3.1/§3.3;
  §7.2 only consumes it.
- **Integrity `[REC]`:** verify each engine binary against a **build-time
  manifest of expected hashes** shipped in-bundle (the same SBOM/checksum data
  §3.7/§6.2 produce). This is a *local* tamper/corruption check (a partially
  extracted portable archive, a truncated download, AV quarantine that gutted a
  file) — **not** a security trust anchor (signing/notarization is out of scope,
  SSOT). `[OPEN→README, defer-able]` Full-hash-every-engine on *every* launch may
  add noticeable startup latency for the heavy office engine; a reasonable
  refinement is hash-on-first-launch-then-cache-a-marker, or presence + a cheap
  size/header check on warm launches. Owner: §7.2 with §3.3.
- **Smoke probe `[REC]`:** optionally, a fast `--version`-style invocation per
  critical engine through the §3.5/§2.12 wrapper to confirm it *runs* on this OS
  (catches a glibc/arch mismatch a hash can't). Kept cheap; gated behind verbose
  mode (§7.5) on warm launches.

**Outcome of a failure:** a missing, corrupt, or non-runnable **required** engine
is an **app-level startup fault** (§2.13) presented in plain language ("A required
conversion component is missing or damaged — please re-download ConvertIA from the
official releases page", with the §7.7 user-initiated link), **never** a stack
trace. A failure of a single engine that only affects *some* formats may instead
degrade to "those formats unavailable" rather than refusing the whole app —
**`[REC]`** mark the affected targets unavailable in the picker (the same surface
the §3.4 patent-gap uses, §5.2) and keep the rest working; classification of
"required vs partial" is owned by §3.1/§2.13, surfaced here.

### 7.2.4 Executable-permission setup (portable build) `[REC]`

On **macOS/Linux**, files extracted from a portable archive may lack the execute
bit; a bundled sidecar that isn't `+x` cannot be spawned. On first launch (idem-
potent on every launch) the core ensures each engine binary is executable:

```rust
#[cfg(unix)]
fn ensure_executable(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = fs::metadata(p)?.permissions();
    if perm.mode() & 0o111 == 0 { perm.set_mode(perm.mode() | 0o755); fs::set_permissions(p, perm)?; }
    Ok(())
}
```

- **macOS quarantine (`com.apple.quarantine`):** because builds are **not**
  notarized/signed (out of scope, SSOT), Gatekeeper may quarantine the bundle and
  block bundled binaries. ConvertIA does **not** attempt to silently strip the
  quarantine xattr (that would be a misleading security gesture); instead the
  fault path (§7.2.3 / §2.13) gives honest first-run guidance, consistent with the
  published-checksum-as-trust-substitute posture (SSOT *Distribution & download
  trust*). The user-facing copy lives with §2.13/§5; the *technical fact* (no
  auto-unquarantine) is owned here.
- **Windows:** no execute-bit concept; bundled `.exe` sidecars run as-is. SmartScreen
  prompts are the analogous unsigned-build friction (out of scope, surfaced on the
  download page per SSOT, not here).

### 7.2.5 First temp-dir creation & orphan reclamation

The per-instance scratch root (§7.1.2 naming) and the log dir (§7.5) are created
on first need. **Startup cleanup** — removing residue from a *previous* crashed/
force-quit run (SSOT *Never harm the original*: "cleaned up on next run") — runs
here but its **mechanism is owned by §2.6** (which roots it must touch, the
per-instance/PID safety check so it never removes a concurrent instance's live
temp). §7.2 only states *when* it runs (step 5, before the window shows) and that
a cleanup that **can't** complete must never let a stale item be reported as a
clean success (it is logged, §7.5, and surfaced per §2.6).

### 7.2.6 macOS TCC file-access prompts `[DECIDED + REC]`

The SSOT default writes output **beside each source** (§2.7), so ConvertIA reads
and writes the user's **Desktop / Documents / Downloads / removable volumes** —
exactly the directories macOS 10.15+ TCC-protects even for **non-sandboxed** apps.
Two concrete facts shape the design:

1. **Usage-description strings are required to *get* a prompt.** For a non-sandboxed
   app, `Info.plist` keys (`NSDesktopFolderUsageDescription`,
   `NSDocumentsFolderUsageDescription`, `NSDownloadsFolderUsageDescription`,
   `NSRemovableVolumesUsageDescription`) must be present so macOS can show the
   one-time "ConvertIA would like to access files in your Desktop folder" dialog.
   These strings ship in the macOS bundle config (Tauri `tauri.conf.json` →
   `bundle.macOS` / `infoPlist`); the **exact phrasing is owned here** as a §7.2
   deliverable (English, SSOT Principle 11) and must read honestly (local
   conversion only).
2. **Child processes do NOT inherit the parent's TCC grant.** A separately-spawned
   sidecar engine (our copyleft-isolation model, §3.6) that opens a protected
   path can hit a TCC denial **even though the GUI app was granted access** —
   macOS attributes TCC to the responsible process, and a plain spawned child is
   not covered by the parent's grant. **`[REC]` Mitigation: the Rust core opens
   the source file itself and hands the engine a file descriptor / a path under
   the app-owned scratch dir (§2.14), rather than handing the engine a raw
   protected user path to open.** I.e. the core (which holds the TCC grant) is the
   only process that touches TCC-protected locations; engines operate on
   core-provided handles/scratch. This dovetails with the §2.14 cross-volume
   strategy (copy-into-scratch then atomic rename) and the §2.12 isolation wrapper.
   The *engine arg/handle plumbing* is owned by §3.5; §7.2 owns the *requirement*
   that engines never be the process that first touches a TCC-protected path.

**Timing:** the first read of a protected location triggers the prompt; ConvertIA
does **not** pre-prompt at launch (no files yet) — the prompt appears naturally at
the first beside-source read/write, which the visible-progress/fail-clearly model
(§1.11/§2.8) already tolerates (a TCC denial maps to the "unreadable / denied
permission" error kind in §2.8, batch continues). **Windows/Linux:** no TCC
equivalent; ordinary filesystem ACL denials map to the same §2.8 error kind.

---

## 7.3 Window & app lifecycle `[DECIDED + REC]`

> SSOT origin: *How It Feels to Use* (visible, cancellable progress), *Never harm
> the original* (no truncated file across an ungraceful end).

### 7.3.1 Window model `[REC]`

- **Single main window**, created by Tauri at startup; label `"main"` (referenced
  by §7.1.1 focus hand-off and §5). No secondary windows in v1; the About screen
  (§5.9) is an in-app view/route, not an OS window.
- **No tray icon, no background/agent mode** in v1 `[REC]`. ConvertIA is a
  foreground tool: closing the window quits the app (it does not lurk in the
  tray). This matches "portable, no installation, no system pollution" — a tray
  resident is closer to an installed service. Closing → quit is the §7.3.3 path.
- **Window size/position:** see §7.4 — **not** persisted in v1 `[REC]`; the window
  opens at a sensible default size each launch.

### 7.3.2 Lifecycle event wiring (Rust)

Two complementary hooks own the lifecycle (per Tauri v2):

- **`WindowEvent::CloseRequested { api, .. }`** (via `WebviewWindow::on_window_event`)
  — fires when the user clicks the window's close control. ConvertIA inspects
  *converter state* and may `api.prevent_close()` to run the §7.3.3 guard. The
  equivalent frontend hook is `getCurrentWindow().onCloseRequested(e => e.preventDefault())`;
  **`[REC]` do the decision in Rust** (the core owns batch state) and only use the
  JS hook to render the confirm UI (§5.2), to avoid a split-brain "is it
  converting?" check.
- **`RunEvent::ExitRequested { api, .. }`** and **`RunEvent::Exit`** (via
  `Builder::run(|app, event| …)` / `on_event`) — `ExitRequested` is the last
  chance to `api.prevent_exit()`; `Exit` is the final cleanup point (flush logs,
  best-effort scratch cleanup — mechanism §2.6).

```rust
.on_window_event(|window, event| {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if converter_is_busy(window.app_handle()) {   // run-level state (§1.9)
            api.prevent_close();
            window.emit("app://close-requested", ()).ok(); // §0.4-owned event → §5.2 confirm UI
        }
    }
})
// …
.run(|app, event| match event {
    RunEvent::ExitRequested { api, .. } => { /* belt-and-suspenders guard */ }
    RunEvent::Exit => { flush_logs(app); best_effort_scratch_cleanup(app); /* §2.6 */ }
    _ => {}
});
```

### 7.3.3 Quit-while-converting `[REC: confirm → cancel cleanly]`

When a batch is **Running** (§1.9) and the user tries to close/quit:

1. The core blocks the close (`prevent_close`) and asks the UI to show a calm,
   keyboard-operable confirm (§5.2/§5.10): **"A conversion is in progress. Quit
   anyway? Files already finished are kept; the one in progress will be
   discarded."** — mirroring the SSOT cancel semantics exactly.
2. **Quit confirmed →** the app performs a **cancel** of the in-flight run using
   the §1.7 cancellation/kill mechanism (process-group kill of the live engine),
   then the §2.6 cleanup (discard the in-progress item's temp, keep finished
   outputs, never touch originals), then exits. This is the *same* code path as an
   in-UI Cancel (§1.11) — quitting is just "cancel + exit". **No new file may
   appear after this point** and at most a discardable temp artifact may remain
   (reclaimed next launch, §7.2.5).
3. **Quit cancelled →** stays open, batch continues.

This guarantee holds **even on an ungraceful end** (OS kill, power loss): the §2.1
atomic-write contract (write-to-temp → atomic rename) means the visible output is
never a truncated file; whatever was mid-flight is at most an orphaned temp under
the per-run scratch dir, reclaimed by §7.2.5. **`[REC]`** the idle state quits
immediately with no prompt (nothing to lose).

### 7.3.4 In-flight queue on close `[DECIDED]`

There is **no persistent queue** and **no resume across launches** (consistent
with §7.4 "persist nothing"): a pending/running queue (§1.9) exists only in memory
for the lifetime of the process. Quitting discards Pending items (they were never
written) and cancels the Running one (§7.3.3). On next launch the user re-drops.
(Resumable batches would require persisting source/target/settings and re-checking
the frozen set — out of v1, parked alongside presets.)

---

## 7.4 Persistence & app state `[REC: v1 persists effectively nothing]`

> SSOT origin: *Local, private & offline* (no accounts, no telemetry), *Portable,
> no installation* (no system pollution), *Future Ideas (Parked)* (presets,
> remembered settings are explicitly parked). README open-question owner: §7.4.

### 7.4.1 Decision

**Recommendation: v1 persists *no user data* and *no cross-session conversion
state*.** Specifically, ConvertIA does **not** persist, by design:

- **No history / no recent-files / no recent-destinations list** — explicit SSOT
  negative (no accounts; presets parked). The end-of-batch summary (§1.12) is
  **session-only**, gone when the window closes.
- **No remembered per-format settings / presets** — parked by SSOT.
- **No window size/position** `[REC]` — opens at a default each launch (a portable
  tool that "leaves nothing behind" is more faithful to the SSOT than remembering
  geometry; cheap to add later if wanted).
- **No resumable queue** (§7.3.4).

**The single, optional exception `[REC]`:** a *tiny* preferences blob holding **at
most** the two purely-cosmetic/convenience values below — never anything derived
from the user's files:

| Key | Type | Default | Why it's defensible |
|-----|------|---------|---------------------|
| `theme` | `"system" \| "light" \| "dark"` | `"system"` | UI preference, not user data; re-asking every launch is annoying. §5.5 owns the theme itself. |
| `lastDestinationMode` | `"beside-source" \| "<absolute path>"` | `"beside-source"` | Re-uses a *chosen* destination (§2.7) across launches; **stores a folder path the user explicitly picked, never a source path or filename**. |

`[OPEN→README]` *Genuine owner call:* whether to ship even this minimal blob or go
**strictly zero-persistence** in v1. **`[REC]` ship the 2-key blob** — it stays
inside "no user data / no history", improves everyday feel, and is trivially
inspectable. If the owner prefers absolute purity, dropping it costs nothing
(theme → always `system`, destination → always `beside-source` each launch).
Whichever way, **a `lastDestinationMode` path must be re-validated as writable at
use time** (§2.7 per-location fallback applies if it has since become read-only/
gone) — it is a *hint*, never a guarantee.

### 7.4.2 If shipped: where it lives & how `[REC]`

- **Mechanism:** the official **`tauri-plugin-store`** (a single JSON file,
  `settings.json`), or a hand-rolled equivalent — either is fine; the store plugin
  is the lower-effort default. Capability `store:default` scoped to the one file
  (§0.10 owns the allowlist entry).
- **Location (per-OS, via Tauri `app.path().app_config_dir()`):**
  - Windows: `%APPDATA%\dev.ne-ia.convertia\settings.json`
  - macOS: `~/Library/Application Support/dev.ne-ia.convertia/settings.json`
  - Linux: `$XDG_CONFIG_HOME/dev.ne-ia.convertia/settings.json` (→ `~/.config/…`)
- **Reconciling with "portable / no system pollution":** this is a single tiny
  cosmetic file in the OS-standard per-user config dir (not the registry, not
  system-wide, not next to the binary), trivially deletable, holding no user data.
  That is the honest reading of "no system pollution" — it is **not** an installer,
  service, or scattered state. `[OPEN→README, minor]` a stricter portable reading
  would put `settings.json` **next to the executable** (true zero-footprint when
  the folder is deleted) — **`[REC]` use the OS config dir**, because a
  beside-binary file breaks when the portable app runs from a read-only medium
  (USB/DMG) and the OS config dir is the cross-platform-correct home; the
  read-only-medium case is exactly why we don't depend on writing beside the
  binary.
- **Failure tolerance:** persistence is **best-effort and never load-bearing** —
  if `settings.json` is unreadable/corrupt/unwritable, ConvertIA logs it (§7.5)
  and runs with defaults; a persistence failure **never** blocks a conversion or
  surfaces as an error to the user. No guarantee (§2) depends on it.

---

## 7.5 Logging & diagnostics (local-only, no telemetry) `[REC]`

> SSOT origin: *Local, private & offline* ("no telemetry", "nothing leaves the
> machine"), *Fail clearly* (no stack traces to the user). Reconcile with §2.11
> (privacy invariant) and §2.13 (fault model). Feeds §6.8 SECURITY/bug-report
> path and the §6.5 reliability gate. README owner: §7.5.

### 7.5.1 Decision: a local, opt-in-verbosity log exists `[REC]`

**Recommendation: ship a local, on-disk log, default level `warn`/`info`,** using
the official **`tauri-plugin-log`** (Rust + a thin JS bridge so frontend errors
also land in the same file). It is **purely local** — written to disk, never
transmitted (no network sink; CSP/allowlist forbid it, §0.10/§2.11). It exists
because §2.13 (app faults), §7.2.3 (engine startup faults), §2.6 (cleanup
failures) and §6.5 (reproducing a one-platform corpus failure) all need a place to
record *what actually happened* without showing the user a stack trace.

### 7.5.2 Targets, location, rotation

- **Targets:** a **rotating file** (the primary diagnostic record) + **stderr** in
  dev. The webview console is *not* a persistence target.
- **Location (per-OS, Tauri `app.path().app_log_dir()`):**
  - Windows: `%LOCALAPPDATA%\dev.ne-ia.convertia\logs\`
  - macOS: `~/Library/Logs/dev.ne-ia.convertia/`
  - Linux: `$XDG_DATA_HOME/dev.ne-ia.convertia/logs/` (→ `~/.local/share/…`)
- **Rotation/retention `[REC]`:** `tauri-plugin-log` `max_file_size` (e.g.
  `5_000_000` bytes) with `RotationStrategy::KeepN` keeping a small number (e.g. 3)
  of files — **bounded total footprint**, so the log can never silently grow
  (consistent with "leave nothing behind" and "no system pollution"). Old files
  are discarded automatically.

### 7.5.3 Redaction stance — reconciling diagnostics with privacy `[DECIDED + REC]`

A log that recorded file **paths** or **contents** would dent the §2.11 privacy
invariant (paths can contain a username, project names, the user's directory
structure). Stance:

- **NEVER logged:** file **contents/bytes**; any decoded data; the **full path** of
  user files at the default level.
- **Default level (`info`/`warn`):** log **structural** facts only — `RunId`/
  `InstanceId`, detected format + count, target + settings, engine name + exit
  code, error **kind** (the §2.8 taxonomy variant), durations, output **basename
  only** (e.g. `vacation.jpg`, never its directory). This is enough to diagnose a
  corpus reliability failure (§6.5) without leaking where the user keeps files.
- **Verbose / "diagnostic" mode (off by default) `[REC]`:** an explicit user
  opt-in (a toggle reachable from About §5.9, or an env var / `--verbose` launch
  flag) that *additionally* records **full paths** and the **exact engine command
  line** (§3.5) for reproduction. Turning it on shows a one-line notice that the
  log will now include file paths and is **still purely local** (nothing is sent).
  This is the deliberate, disclosed trade: privacy by default, full reproducibility
  on demand — and it never changes the no-network property.
- **No automatic upload, ever.** The §6.8 `SECURITY`/bug-report flow asks the user
  to *attach* the log file **manually** to a report; ConvertIA neither reads it
  back nor transmits it. "Phone home" stays impossible (§2.11).

### 7.5.4 Dev-facing diagnostics (makes §6.5 operable)

In verbose mode the log additionally captures: the exact spawned argv per engine
(§3.5), engine `stderr` (captured-and-classified by §2.13, here also persisted),
the resolved scratch/temp paths (§2.14), per-item timing, and the chosen
output-plan decisions (§1.8) incl. any per-location divert. This is what lets a
maintainer reproduce a "fails only on Linux for this PDF" corpus item (§6.4/§6.5)
from a user-supplied log without remote access.

---

## 7.6 Update posture (no auto-updater) `[DECIDED: no phone-home]`

> SSOT origin: *Local, private & offline* ("does **not** check for updates or
> phone home"; "any future update check would be opt-in and disclosed, never
> silent"), *Distribution & download trust* (canonical GitHub Releases).

### 7.6.1 The Tauri updater is explicitly absent `[DECIDED]`

Concrete spec items (each a Phase-3 checklist line, asserted by §6.5/§2.11):

- **`tauri-plugin-updater` is NOT added** to `Cargo.toml` / the Builder. There is
  no updater endpoint, no update manifest, no pubkey in `tauri.conf.json`, no
  `updater` bundle config. Its *absence* is the implementation.
- **No background/startup version check** of any kind (§7.2.2): the shell makes
  zero network calls. There is no "you're up to date" banner, no silent fetch.
- **CSP / capabilities (§0.10)** allow **no remote origins** and the HTTP/updater
  permissions are not granted, so even an accidental fetch is blocked at the
  WebView boundary — defense in depth behind the policy decision.

### 7.6.2 How the user learns of a new release `[DECIDED]`

- The current version (from the build, `app.package_info().version` / Cargo
  `CARGO_PKG_VERSION`) is **displayed** in the About screen (§5.9; §7 supplies the
  value, §5.9 renders it).
- About offers a **user-initiated** link to the canonical Ne-IA GitHub Releases
  page (the only authentic source, SSOT). Clicking it is a §7.7 shell-out (open
  URL in the default browser) — the *only* permitted, *explicitly* user-triggered
  network action. ConvertIA does not fetch or parse that page itself.

### 7.6.3 Future opt-in check (parked) `[DEFER]`

If a future version ever adds an update check it must be **opt-in, disclosed, and
visible** (SSOT). The §7.4 persistence design leaves room for a single future
`updateCheckOptIn: boolean` key (default `false`); it is **not** present in v1.

---

## 7.7 OS shell-out (open-folder / open-file / open project page) `[REC]`

> **Single owner** of the concrete shell-out operations behind the DoD core-UX
> gate (one-click open-folder/file) and the only permitted network (user-initiated
> open-project-page). On the §0.10 capabilities allowlist (opener scope). §2.7
> fixes *which* path; this owns *how* the shell-out works. §1.12 produces the
> output→source mapping it consumes; UI entry via §5.3 `OpenActions`. README
> owner: §7.7.

### 7.7.1 Mechanism: `tauri-plugin-opener` `[DECIDED]`

All shell-out goes through the official **`tauri-plugin-opener`** (the v2
successor to the old `shell.open` allowlist). Three operations, exposed as IPC
commands (the canonical command names/payloads are enumerated by §0.4 — listed
here by behaviour, not re-specified):

| Operation | Plugin API | Behaviour | Used by |
|-----------|-----------|-----------|---------|
| **Reveal output in file manager** | `reveal_item_in_dir(path)` (Rust `opener::reveal_item_in_dir`, JS `revealItemInDir`) | Opens the OS file manager **with the file selected/highlighted**: Explorer `/select,` on Windows, Finder reveal on macOS, best-effort folder-open on Linux | "Open folder" (§5.3) |
| **Open a finished output** | `open_path(path)` (JS `openPath`) | Opens the converted file in the OS default app for its type | "Open file" (§5.3) |
| **Open the project/releases page** | `open_url(url)` | Opens the canonical Ne-IA URL in the default browser | About link (§5.9 / §7.6.2) |

**"Open folder" target (per SSOT *How It Feels* 8):** opens the **common root of
the dropped selection** (the mapping is owned by §1.12/§2.7); for the beside-source
default that is the dropped folder, for a chosen-destination it is that
destination root. On Windows/macOS the reveal API additionally highlights the
specific output when a single file is the subject; Linux file managers vary, so
the **`[REC]`** fallback is "open the containing directory" (no reliable
cross-distro select).

### 7.7.2 Allowlist scope (§0.10 owns, §7.7 constrains) `[REC]`

The opener capability must be **scoped, not blanket** (§0.10 owns the exact
manifest; §7.7 states the required scope):

- **`opener:allow-reveal-item-in-dir` / `opener:allow-open-path`:** scoped to paths
  ConvertIA legitimately produces — i.e. **outputs under a destination the app
  itself planned** (§1.8/§2.7) and the **dropped roots**. Recommended scope
  pattern restricts to the output/destination trees rather than `**/*` everywhere;
  the binding glob is set in §0.10. The point: the WebView can ask to reveal *an
  output we made*, not arbitrary system paths.
- **`opener:allow-open-url`:** scoped to **`https://` only**, and **`[REC]`** to the
  fixed canonical Ne-IA host(s); the project page is a constant, not user input, so
  the URL need not be a free parameter at all (the command can take **no URL
  argument** and open a compiled-in constant — strongest option). **`[REC]` open
  the compiled-in constant**, eliminating any URL-injection surface from the
  WebView.

### 7.7.3 Open-file safety `[DECIDED + REC]`

Launching an **external** application on a **fresh, possibly-untrusted** artifact
is security-relevant (§0.11 maps this threat to §7.7). Constraints:

- **Only ConvertIA-produced outputs are openable** via these actions — never an
  arbitrary path from the WebView, never a *source* file, never an engine
  intermediate. The command **validates** the requested path against the current
  `RunResult`'s recorded outputs (§1.12) before invoking the opener; a path not in
  that set is refused (and logged, §7.5). This makes "open file" structurally
  unable to launch anything other than a file the user just chose to create.
- **It hands off to the OS default app** (we do not pick the program, except the
  browser-for-URL case) — ConvertIA is not in the business of choosing handlers;
  the output's *type* is one the user explicitly converted *to*, so opening it is
  expected. There is **no auto-open**: the file is opened **only** on an explicit
  click of "Open file" (§5.3) — never automatically at end-of-batch — so the user
  consciously chooses to launch a freshly produced artifact.
- **Reveal-in-folder is preferred as the default affordance `[REC]`:** "Open
  folder" (reveal, which does **not** execute the file) is the safer primary
  action; "Open file" is offered but secondary. This keeps the common, safe path
  one click away while making the act-of-launching deliberate.

---

## 7.8 OS intake & integration posture (Open-with / launch args; explicit negatives) `[REC]`

> SSOT origin: *How It Feels to Use* (intake), *Future Ideas (Parked)* (drag-out
> parked), *Local, private & offline*. Mirrors the §7.4 explicit-negatives stance.
> Feeds §1.1 (intake) and binds to §7.1 (single-instance) and §2.4 (freeze point).

### 7.8.1 Launch-time / OS open-file intake `[REC: accept, route through §1.1]`

ConvertIA **accepts** paths that arrive via OS launch entry points and routes them
into the **same** intake path as a drop/picker — so the frozen-source-set (§2.4)
and one-batch-at-a-time (§1.3) rules apply identically. The entry points:

- **macOS:** the open-documents AppleEvent (the OS delivers files to the running
  app). Tauri surfaces this; the single-instance plugin (§7.1.1) ensures it lands
  in the one instance.
- **Windows:** files passed as **`argv`** to the process. Captured in the
  single-instance callback (§7.1.1) for a second launch, and read at first launch
  in `setup` (`std::env::args_os`).
- **Linux:** the desktop-entry **`%F`/`%U`** field expansion → `argv`, handled the
  same as Windows.

```rust
// One funnel for every launch-time path source → §1.1 frozen-set builder.
fn forward_launch_intake(app: &AppHandle, argv: &[String], cwd: &str) {
    let paths = parse_path_args(argv, cwd);     // resolve relative → absolute
    if paths.is_empty() { return; }
    app.emit("app://intake", paths).ok(); // §0.4-owned event; UI mirrors a drop (§5.2/§1.1)
}
```

**Interaction with single-instance + freeze (§7.1.1 / §2.4):** at first launch the
paths seed the idle state as if dropped. A *second* launch's paths arrive via the
single-instance callback; if the primary instance is mid-conversion, the §7.1.1
`[REC]` (refuse with "busy") applies — the new paths are **not** silently merged
into the frozen set of a running batch (that would violate §2.4 "files appearing
after the freeze are never ingested"). When idle, they start a fresh drop.

### 7.8.2 Explicit negatives (mirroring §7.4) `[DECIDED]`

ConvertIA deliberately does **not**, in v1:

- **Register any file associations / default-handler claims.** ConvertIA never
  installs itself as the handler for `.heic`, `.docx`, etc. — it is a portable,
  no-installation tool (SSOT) and claiming handlers is system pollution that
  outlives the app. "Open with → ConvertIA" works only if the *user* chooses it
  ad-hoc via the OS; we publish no association. (This also avoids the unsigned-app
  default-handler friction on Windows/macOS — out of scope, SSOT.)
- **Register any URL scheme / deep link** (no `convertia://`). No deep-linking
  plugin. (The single-instance plugin is used purely for instance dedup + path
  hand-off, §7.1.1, not for scheme handling.)
- **Provide drag-out / "drag a finished result into another app".** SSOT parks
  drag-out under *Future Ideas*. The WebView cannot originate a real
  filesystem-path drag anyway (§0.4); v1 does not attempt it.
- **Provide clipboard export of results** (copy file / copy image to clipboard).
  Not in scope for v1; the only output OS-integration is the §7.7
  open-folder/open-file actions.
- **Run as a service / login item / scheduled task / shell-extension** (no Explorer
  "Convert with ConvertIA" context-menu, no Quick Action). All are installed,
  persistent OS integrations contrary to "portable, no installation, no system
  pollution".

These negatives are intentional scope boundaries, not gaps: output OS-integration
is **exactly** §7.7 (reveal-in-folder / open-file), input is **exactly** drop +
picker + keyboard (§1.1/§5) plus the ad-hoc launch-time intake above (§7.8.1).

---

## Open items surfaced by this section (for the README log)

- **§7.1.1** — second-launch hand-off while a batch is **running**: queue-after vs
  refuse-busy. `[REC]` refuse-busy. Owner: §7.1.
- **§7.2.3** — engine integrity: hash-every-launch vs hash-once-cache (startup
  latency vs assurance). `[REC]` hash-on-first-launch + cheap warm-launch check.
  Owner: §7.2 with §3.3.
- **§7.4.1** — persist the minimal 2-key prefs blob (theme +
  last-destination-mode) vs strict zero-persistence. `[REC]` ship the blob.
  Owner: §7.4.
- **§7.4.2** — prefs file in OS config dir vs beside-binary (portability reading).
  `[REC]` OS config dir. Owner: §7.4 (minor).
- **§7.5.1/§7.5.3** — ship a local log at all, and the verbose-mode opt-in for
  full-path/command-line capture. `[REC]` yes to both, privacy-by-default. Owner:
  §7.5.
