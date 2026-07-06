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
> the README open-questions log) · `[REC]` a recommended default that, **per the owner's
> standing mandate, is ADOPTED AS DECIDED** (sensible-default sections — instance/run
> identity §7.1, sidecar verification §7.2.3, window model §7.3.1, persistence §7.4,
> logging §7.5, OS shell-out §7.7, intake posture §7.8 — are all decided; the `[REC]`
> marker is retained only to show the call originated as a recommendation) ·
> `[DEFER]` settled during implementation. **No section in 07 is genuinely `[OPEN]`.**

---

## 7.1 Instance & run identity `[REC]`

> SSOT origin: *Never harm the original* ("a second app instance",
> "another instance's in-progress file", "outputs landing in a source folder do
> not expand or restart the batch"). Load-bearing for §2.6 (per-run/instance temp
> ownership), §2.4 (frozen source set), §2.14 (scratch), §0.9 (subprocess pool).

### 7.1.1 Single-instance policy `[REC: single-instance, hand-off]`

**Recommendation: ConvertIA runs as a single GUI instance per OS user session,**
using the official **`tauri-plugin-single-instance`** (v2) (the per-OS-user scope holds on **Windows/Linux**; **macOS** is machine-global — see the macOS caveat below + §0.11 **T13**). Rationale:

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
    // origin = SecondInstance; the §7.8.1 funnel enforces the refuse-busy gate (below).
    forward_launch_argv(app, &argv, &cwd, IntakeOrigin::SecondInstance); // → §7.8 → §1.1 intake
}))
// then the remaining §0.8 plugins are registered in the same Builder chain:
//   .plugin(tauri_plugin_dialog::init())   // §0.4.1 C2a/C2b native pickers via DialogExt
//   .plugin(tauri_plugin_opener::init())   // §7.7 open-folder/file/url via OpenerExt
//   .plugin(tauri_plugin_store::Builder::default().build())  // §7.4 settings.json
//   .plugin(tauri_plugin_log::Builder::new()...build())      // §7.5 rotating log
```

**Plugin registration order `[DECIDED]`:** `tauri-plugin-single-instance` is registered
**first** (it must win before any window is created); the remaining §0.8 plugins —
`tauri-plugin-dialog` (the C2a/C2b native pickers, called Rust-side via `DialogExt`, §0.4.1),
`tauri-plugin-opener` (§7.7), `tauri-plugin-store` (§7.4) and `tauri-plugin-log` (§7.5) —
follow in the same Builder chain. `tauri_plugin_dialog::init()` is **required** for
`app.dialog().file().pick_file(..)` / `.pick_folder(..)` to exist in the C2a/C2b handlers
(without it both pickers fail to compile).

- On **Windows/Linux** this is the only way a second launch (and any OS-passed
  file argv, §7.8) reaches the primary instance — without it the OS spawns a new
  process. On **macOS** the OS already routes a second open to the running app via
  the open-documents AppleEvent (§7.8); single-instance is kept for parity and to
  cover direct binary re-execution of the portable build. **`[REC]` macOS
  single-instance behaviour is a §6.6 verification item** — `tauri-plugin-single-
  instance`'s macOS path is the least-mature of the three (the AppleEvent route does
  most of the work there), so the §6.6 macOS walkthrough must confirm a second launch
  / Open-with re-focuses the running instance and hands paths off, rather than starting
  a competing converter.
- **Multi-user / fast-user-switching:** the lock is **per OS user**, not
  machine-global (the plugin's default lock scope), so two different logged-in
  users may each run their own instance — acceptable because their temp/scratch
  (§2.14) and output locations are user-scoped anyway. **macOS caveat `[DECIDED]`:**
  this per-OS-user scope holds on **Windows** (per-Session `CreateMutexW`) and
  **Linux** (session D-Bus) but is **NOT achievable on macOS** with this plugin (its
  socket is machine-global `/tmp`) — see the accepted-limitation note below + the
  §0.11 **T13** threat class.

**`[DECIDED]` second-launch hand-off while mid-conversion = refuse-busy (option b).**
When the primary instance is **mid-conversion**, a second launch's paths are
**refused** with a calm "ConvertIA is busy — finish or cancel the current batch
first" note (rather than silently queued as a deferred drop). This keeps the freeze
point (§2.4) and the one-batch-at-a-time model (§1.3) unambiguous and avoids a hidden
queue the user can't see. (Adopting the standing [REC].) When **idle**, a second
launch's paths start a fresh drop normally. **The single-instance callback is the
PRIMARY refuse-busy gate `[DECIDED]`:** when busy the hand-off dies at the §7.8.1
funnel — **no `PendingIntake` stash, no `app://intake` nudge** (and per the 2026-07-06
owner ruling (core-owned paths) the event is a **payload-less nudge** in any case: it
never carries paths, busy or idle, so no path can ever reach the UI). The §5.8 UI
guard (a nudge arriving in a state that cannot take fresh intake renders the
`BusyNotice`, §5.4) is **defence-in-depth**, not the primary gate. **UI surface = the `BusyNotice` Banner (§5.3), rendered under the
`AppHeader`** when that defence-in-depth guard fires (§5.5 app-chrome layout); it is a
passive non-modal notice, never a modal. **The primary busy SIGNAL is the window
re-focus itself `[DECIDED]`:** the single-instance callback **re-focuses the running
window** (the OS brings ConvertIA forward) — that re-focus, landing the user back on the
live `Converting` screen, IS the sufficient primary "we're busy" feedback. **Mid-run, the
`BusyNotice` text is shown ONLY on the defence-in-depth path** (if/when a nudge somehow
reaches the UI mid-run despite the core-side gate — and it carries nothing, so it could
not set-swap even then; the §5.4 non-intake states outside a run have their own Banner
trigger, §5.8). A Phase-3 dev must **not** add
a new event/toast to announce busy-ness — re-focus + the (rare) defence-in-depth Banner
are the whole surface, so the §0.4.2 three-`app://`-event invariant is not expanded.

**macOS unsigned two-copies edge case = accepted v1 limitation `[DECIDED]`.** Per the §6.6
macOS sub-test, on **unsigned builds launched from two *separately-extracted* copies of the
`.app`** (e.g. the user unzipped the download twice into different folders), the
`tauri-plugin-single-instance` macOS path may **not recognise both as one instance** (the
plugin's macOS single-instance is the least-mature leg, §7.1.1 [REC] above). This is an
**accepted v1 edge case — documented, not a code fix**: it only arises from the unusual
"two separate extracted copies" action; the **normal single-`.app` AppleEvent path is
single-instance-correct** (Open-with / re-launch of the one installed copy re-focuses the
running instance, §7.8.1). v1 does **not** add defensive bundle-ID locking code to chase
this corner — it is recorded here so Phase 3 does not build unnecessary hardening for it.
The §6.6 walkthrough confirms the normal one-`.app` path; the two-copies case is noted as a
known limitation on the download page if it proves to matter.

**macOS multi-user machine-global single-instance = accepted v1 limitation `[DECIDED]`.**
`tauri-plugin-single-instance`'s macOS path hard-codes its single-instance socket at
**`/tmp/{id}_si.sock`** (the plugin source; `/tmp` is world-writable + machine-global on
macOS), so the **"per OS user, not machine-global" scope above is NOT achievable on macOS with
this plugin** (Windows = per-Session `CreateMutexW`, Linux = session D-Bus, are per-user; macOS
is the sole gap — the plugin exposes no API to relocate the socket). On a **multi-user Mac** two
logged-in users share `/tmp/{id}_si.sock` (which carries the launch `cwd`+`argv`): the second
user may not get their own instance, and the shared socket is a **local cross-user surface** (a
different user can pre-bind to receive this user's launch paths, inject paths into the intake, or
squat to break single-instance). **Accepted for v1, documented as threat class §0.11 T13 (not a
code fix):** a local logged-in second macOS user is out of the offline-converter threat model;
the injection half is bounded by the same §2.4 freeze re-validation as T2b (a substituted path
only converts an A-readable file to an output beside it, no-clobber + link-safe); the leaked data
is **user-visible launch PATHS**, not file contents; single-user Macs are the dominant config; and
the **macOS PRIMARY single-instance path is the AppleEvent (§7.8)**, unaffected — the plugin's
/tmp socket covers only direct-binary re-exec (the least-mature leg). v1 does **not** add a
per-user-`$TMPDIR` macOS socket (the §0.11 T13 row records that option as the heavier path not
taken); the §6.6 macOS walkthrough confirms the normal single-`.app` AppleEvent path is
single-instance-correct.

### 7.1.2 `InstanceId` and `RunId` model `[DECIDED]`

These two ids are the entities §0.6 lists as "defined in §7.1". Both are
**process-local, never persisted, never networked** (§2.11).

| Id | Type | Scope / lifetime | Derivation | Purpose |
|----|------|------------------|------------|---------|
| `InstanceId` | `Uuid` (v4) — opaque 128-bit | One running process, created once in `setup` | Random at launch | Names the per-instance scratch root (§2.14) and stamps temp artifacts so startup cleanup (§2.6) can tell *this* instance's residue from a *different* instance's still-running temp |
| `RunId` | `Uuid` (v4) | One "drop → … → summary" cycle (one `Batch`); a new drop after a summary starts a new `RunId` | Random when **`start_conversion` (C6) accepts the batch** (§0.4.1 C6 / §0.4.4); the §2.4 freeze produces the **`CollectedSetId`** (the pre-run identity), **not** the `RunId` — the `RunId` is minted only when CONVERT begins, so the per-run scratch `run-<RunId>/` (§2.6.1) never exists before any RunId is minted | Owns the per-run temp subdir; cancellation/cleanup (§2.6), progress events (§0.4) and the end-of-batch summary (§1.12) are all keyed by it |

Pseudo-types (mirrored to TS via the §0.4.5 mechanism — not re-decided here):

```rust
pub struct InstanceId(pub Uuid);   // app-managed singleton via app.manage(...)
pub struct RunId(pub Uuid);        // field of `Batch`/`RunResult` (§0.6)
```

**Scratch-root naming (the load-bearing detail §2.6/§2.14 depend on):** the
per-instance scratch root is named with **both** the `InstanceId` **and the OS
PID**, e.g. `…/convertia/scratch/<InstanceId>.<pid>/`. Per-run subdirs live under it
as `…/<InstanceId>.<pid>/run-<RunId>/`. (Exact path policy is owned by §2.14; this
section only fixes the *identity* embedded in it.)

> **Liveness predicate — the advisory lock is authoritative, the PID is a label
> `[DECIDED]`.** The PID in the dir name is a **human-readable hint / fast
> pre-filter only**; it is **not** the liveness test, because PIDs are reused (a dead
> instance's PID may now belong to an unrelated live process → false "alive", or a
> wrapped/re-execed process changes PID → false "dead"). The **single authoritative
> liveness predicate** is the **§2.6.3 advisory lock** (`run-<RunId>/.lock` held via
> Unix `flock`/`fcntl` / Windows `LockFileEx` for the run's lifetime): lock-held ⇒
> live ⇒ never reclaimed; lock-free/stale ⇒ dead ⇒ reclaimable. §2.6.3 owns the
> mechanism; §7.1 only supplies the identity it locks. So PID-alive is **never** used
> as the predicate (reuse race); the held-lock is.

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
   §0.3.1). A missing/old WebView is a §7.2/§2.13 startup fault **where the core can
   observe it** (macOS WKWebView / Linux WebKitGTK init failures the Rust core sees);
   the **Windows WebView2-*absent* portable case is the honest exception** (§0.3.1) —
   the loader fails before the core runs, so there is no in-app fault to show and the
   "fail clearly" substitute is the §6.2.4 download-page prerequisite note, not a dialog.
7. **Process launch-time intake** (§7.8): if the app was opened *with* file paths
   (OS open-doc / argv), feed them through the §7.8.1 funnel (stashed core-side; the
   frontend's mount-time C1 drain collects them into §1.1 once the window is ready).
8. Hand to UI empty/idle state (§5.2).

Steps 3–5 run in the Rust core during `setup`/just after; the window is only shown
once they succeed, so a hard fault is shown as a clean fault screen (§2.13), never
a half-broken UI. **Mechanism `[DECIDED]` (P2.106.6 / P2.109):** the single `main`
window is config-declared **`visible: false`** in `tauri.conf.json` (created hidden,
not by a programmatic builder — §7.3.1), and the core reveals it
(`get_webview_window("main")` → `.show()`) **only on the readiness-gate success path**
(steps 3–5 `Ok`); a readiness fault instead skips this normal reveal and hands the
app-level `AppFault` to the §2.13.3 presentation. `get_webview_window("main")`
returning `None` at step 6 is the core-observable WebView-init fault seam (missing/old
WKWebView / WebKitGTK, §0.3.1) — **P2.109 builds that detection + routing** (the `None`
arm constructs a `WebviewFault` `AppFault` and routes it to `present_startup_fault`).

**Which surface a startup fault renders on is `[DECIDED]` by the WebView's own health
(P2.109) — the fault channel splits in two:**

- **Readiness faults (steps 3–5): `EngineMissing` / `BundleDamaged`** leave the WebView
  itself healthy, so they present over the **built** §0.4.2 `app://fault` event → the
  §5.8 WebView fault screen. Because such a fault can fire **before** the §5.8 listener
  is registered (the same first-frame race the §7.8.1 launch-intake buffer closes), it
  is replayed through a **`PendingFault` buffer** on listener-ready. The `app://fault`
  emit + `PendingFault` buffer body lands with the step-3–5 verifier bodies (**P4**),
  not P2.109.
- **The WebView-init fault (step 6): `WebviewFault`** — the OS WebView runtime could not
  create the view — makes an `app://fault`→WebView emit **impossible** (there is no
  WebView to render it), so it presents on a **native surface** (not the WebView; the
  concrete native mechanism is a P4 decision, §2.13.3). The Windows WebView2-*absent*
  case is **not** this: it fails **before** the core runs (§0.3.1 honest exception), so
  the core never observes it.

`present_startup_fault` is the mechanism-independent §2.13.3 entry point both channels
route through; it records the fault locally (§7.5) now, and the two presentation bodies
above are P4.

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

- **Presence (out-of-band — iterates the BINARY list, NOT the `trait Engine`
  registry) `[DECIDED]`:** the presence/integrity loop iterates the **§3.3.1 expected
  bundled-binary list** (the `bundle.externalBin` + resource binaries — `ffmpeg`,
  `ffprobe`, `soffice`, `pdftotext`, `pandoc`, `convertia-imgworker`), resolving each
  path (under the Tauri resource dir / sidecar location, §0.7) and confirming the file
  exists. It does **NOT** iterate the §3.2.3 `trait Engine` registry and does **NOT**
  call `descriptor()` — so an engine `EngineId` that has **no `trait Engine` impl** (the
  non-trait variants `FFprobe` and `ImageMagick`, §0.6) is reached purely through this
  binary list, never through `descriptor()`. The authoritative *list* of expected binaries
  per platform is owned by §3.1/§3.3; §7.2 only consumes it. **The binary name per
  `EngineId` comes from the §3.3.1 externalBin entry** (e.g. `EngineId::FFprobe` →
  `binaries/ffprobe`), not from any trait method.
  - **Names are BARE runtime names, NOT target-triple-suffixed `[DECIDED]`.** The presence
    loop checks the **bare runtime names** — `ffmpeg`, `ffprobe`, `soffice`, `pdftotext`,
    `pandoc`, `convertia-imgworker` — matching the §3.3.3 `current_exe().parent()` resolution
    (Tauri strips the `-<target-triple>` suffix at bundle time; the suffix is a build/stage
    artifact only). **On Windows append `.exe`** to each. Checking the suffixed
    `ffmpeg-x86_64-unknown-linux-gnu` name at runtime would **always report missing** — the
    loop must use the stripped names that actually ship beside the app exe.
  - **`FFprobe` presence-checked, health rolled into FFmpeg `[DECIDED]`.** `ffprobe`
    ships alongside `ffmpeg` (same upstream, same GPL build, §3.1 row 2 / §3.3.1) and is
    the video two-phase probe binary (§3.2.1). It is **presence + integrity checked as its
    own binary** (`binaries/ffprobe`) via this out-of-band loop, but — like ImageMagick —
    it has **no standalone `EngineStatus` row in the C12 surface**: its availability is
    rolled into the FFmpeg engine's status (a missing/corrupt `ffprobe` makes the FFmpeg
    `EngineStatus.runnable = Some(false)`, since no video job can probe without it). Its
    `EngineId::FFprobe` appears in the SBOM/NOTICE layer (§3.7) and this binary
    presence/integrity loop, never in the §3.2.3 registry.
- **Integrity `[DECIDED]`:** verify each engine binary against a **build-time
  manifest of expected hashes** shipped in-bundle (the same SBOM/checksum data
  §3.7/§6.2 produce). This is a *local* tamper/corruption check (a partially
  extracted portable archive, a truncated download, AV quarantine that gutted a
  file) — **not** a security trust anchor (signing/notarization is out of scope,
  SSOT). **Strategy `[DECIDED]` (adopting the [REC]): hash-on-first-launch, then
  cache a marker; on warm launches do presence + a cheap size/header check**, not a
  full re-hash of the heavy office engine each time (avoids startup latency). A full
  re-hash is triggered only when the marker is absent (first launch / post-update).
  **First-launch hash-cache deliverable `[DECIDED]`:** the cache is a **small JSON marker
  file `engine-integrity.json` in the OS config dir next to the prefs blob** (Tauri
  `app_config_dir()`, e.g. `~/.config/dev.ne-ia.convertia/` — a **separate file**, not
  merged into the 3-key prefs blob, so a prefs reset never forces a re-hash). It records,
  per engine, `{ id, expected_hash, expected_size, app_version }`. **Warm-launch
  validation:** if the marker is present **and its `app_version` matches the running
  build**, do presence + the cheap size/header check only; if the marker is **absent or
  `app_version` differs** (first launch or post-update), re-hash all engines and rewrite
  the marker; a size/header mismatch on a warm launch forces a re-hash of that engine.
  **The "cheap size/header check" is concrete `[DECIDED]`:** **(a) file size equals the
  marker's `expected_size`** AND **(b) the first N bytes match the expected executable
  magic for the platform** — **ELF `0x7F 45 4C 46`** (Linux), **PE `MZ` (`0x4D 5A`)**
  (Windows), **Mach-O / fat `0xCA FE BA BE` (fat) or `0xCF FA ED FE` (64-bit thin)**
  (macOS). **The magic-byte check (b) applies ONLY to the EXECUTABLE sidecars `[DECIDED]`**
  (the §3.3.1 `externalBin` binaries — `ffmpeg`/`ffprobe`/`soffice`/`pdftotext`/`pandoc`/
  `convertia-imgworker`). **`soffice` magic is platform-conditional `[DECIDED]`:** on
  **Linux** the bundled `soffice` is a **`#!` shell-script wrapper, NOT an ELF** (it `exec`s
  the real `soffice.bin` ELF in the program tree), so its magic check is a **shebang check
  (`0x23 0x21` = `#!`) / script-type check**, **not** the ELF magic (an ELF check on it would
  always fail); the actual LibreOffice ELF binaries (`soffice.bin` etc.) live in the program
  tree and are covered by the size-only warm check + first-launch full re-hash like the other
  program-tree files. On **macOS** `soffice` is a **Mach-O** (standard Mach-O magic applies);
  on **Windows** `soffice.exe` is a PE. All other executable sidecars use the standard
  per-platform magic. **Non-binary bundled resources (the bundled fonts §3.9, the
  LibreOffice program-tree data files, NOTICE/licence text) have NO single executable magic**,
  so for them the warm-launch check is **size-only** (size equals `expected_size`); their
  full content is covered by the **first-launch / version-change full re-hash** like every
  other resource. This catches truncation/AV-gutting/partial-extract cheaply without a full
  re-hash; it does **not** catch same-size in-place corruption (only the full re-hash on
  first-launch / version-change does — an accepted limitation, since runtime is not a
  tamper anchor, §0.11 T3). Owner: §7.2 with §3.3.
- **Smoke probe `[REC]`:** optionally, a fast `--version`-style invocation per
  critical engine through the §3.5/§2.12 wrapper to confirm it *runs* on this OS
  (catches a glibc/arch mismatch a hash can't). Kept cheap; gated behind verbose
  mode (§7.5) on warm launches.

**`EngineHealth` — the C12 return (defined here; §0.4.1 C12 references it).** The
cached result of this startup probe. Feeds §5.2 (disable/omit unavailable targets)
and the §7.2.4 startup-fault surface. Owned by §7.2:

```rust
struct EngineHealth {
    engines: Vec<EngineStatus>,        // one per registry-eligible engine (FFmpeg, LibreOffice,
                                       //   Poppler, Pandoc, ImageCore, NativeCsvTsv). The non-trait
                                       //   delegate/probe binaries (FFprobe, ImageMagick) get NO
                                       //   standalone row — their presence/integrity (checked via
                                       //   the §7.2.3 out-of-band binary loop) is rolled into the
                                       //   owning engine's status (FFprobe→FFmpeg, ImageMagick→
                                       //   ImageCore).
                                       // NativeCsvTsv is NOT in the §3.3.1 binary list (it is
                                       //   InProcessNative, §3.5.6 — no sidecar file to resolve),
                                       //   so the §7.2.3 presence/integrity LOOP does not produce
                                       //   its row. Its EngineStatus is SYNTHESIZED `[DECIDED]`:
                                       //   `{ present: true, integrity_ok: true, runnable: Some(true) }`
                                       //   (always-available-in-core, pure-Rust, nothing to verify)
                                       //   — appended after the loop, never from it.
    unavailable_targets: Vec<TargetId>,// §3.4 patent-gapped on THIS platform (PlatformUnavailable)
    all_critical_ok: bool,             // derived: every required engine present+runnable
}

struct EngineStatus {
    id: EngineId,                      // §0.6
    present: bool,                     // file resolved at its expected path
    integrity_ok: bool,               // matched the build-time hash manifest (§7.2.3)
    runnable: Option<bool>,            // Some(result) if the smoke probe ran; None if skipped
}
```

> **ImageMagick has no standalone `EngineStatus`, but its delegate IS smoke-probed
> `[DECIDED]`.** ImageMagick is a libvips
> **delegate linked inside the image-worker** (§3.1 row 1d), not a sidecar with its own
> file to resolve, so it gets **no per-engine `EngineStatus` row**; its availability is
> rolled into the image-worker's (`EngineId::ImageCore`) health. Its `EngineId::ImageMagick`
> appears **only** in the SBOM/NOTICE layer (§3.7), never in the §3.2.3 registry or this
> presence-check loop — consistent with §3.1's "delegate, not a registry engine".
> **BUT** ImageMagick is **REQUIRED for BMP load+save** (§3.1 row 1d — not a
> fallback), so a present-but-broken/missing delegate would otherwise fail **every BMP
> conversion silently at first use at runtime**, not at startup. (**ICO save** is the
> `magicksave` default but `[DEFER: build spike]` §3.5.5 — if the spike fails, ICO save uses
> the in-core Rust assembler and does **not** depend on the ImageMagick delegate.) To surface
> a missing BMP delegate as a **startup fault** instead, the image-worker smoke probe (§7.2.3
> above) **MUST include a BMP delegate exercise `[DECIDED]`** — e.g. a tiny
> `magicksave`/`magickload` BMP round-trip **or** a `vips`/ImageMagick `--list-formats`-style
> check verifying **BMP is a registered delegate** (and, **if the magicksave ICO path ships**,
> ICO too) — so a missing/corrupt ImageMagick delegate makes the `ImageCore`
> `EngineStatus.runnable = Some(false)` (and BMP targets show as unavailable, §5.2) at
> startup, never a silent per-item failure on the first BMP job.

**`AppInfo` — the C11 return (defined here; §0.4.1 C11 references it; §5.9 displays
it; the licence/NOTICE data is generated by §3.7).** No network; all data is
in-bundle:

```rust
struct AppInfo {
    version: String,                   // semver, e.g. "1.0.0"
    build_id: String,                  // CI build identifier (§6)
    platform: Platform,                // §3.2 (Win | MacOS | Linux)
    third_party_notice: String,        // the §3.7 THIRD-PARTY-LICENSES.txt contents (bundled)
}
```
  - **macOS ordering caveat `[REC]`:** on macOS Sequoia a quarantined/Gatekeeper-
    blocked bundled binary (§7.2.4 — builds are unsigned, and **each sidecar is
    independently quarantined**) makes the spawn itself fail. To ensure that fault
    surfaces **in a window** (not as a silent pre-window hang), the macOS smoke probe
    is **deferred until after the WebView window is shown** (step 6), or **downgraded
    to presence + hash only on first launch** with the runtime-spawn check happening
    lazily on the first real conversion. Either way the quarantine fault becomes a
    visible **`QuarantinedByOs`** (§2.8) message guiding the user to Privacy & Security
    → "Open Anyway", **never** a blank window — and it is distinguished from a genuinely
    missing/corrupt engine (`EngineMissing`/`BundleDamaged`).

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

- **macOS quarantine (`com.apple.quarantine`) — Sequoia reality `[DECIDED]`:**
  because builds are **not** notarized/signed (out of scope, SSOT), Gatekeeper
  quarantines the bundle and blocks both the app and **each independently-quarantined
  bundled sidecar** (FFmpeg, LibreOffice's `soffice`, pdftotext, pandoc, the image
  worker). On **macOS Sequoia (15.x) the old Control-click "Open" bypass was removed**:
  the user must, on a blocked launch, go to **System Settings → Privacy & Security →
  "Open Anyway"**. Critically, **approving the app does not approve the sidecars** — so
  even after ConvertIA opens, the **first conversion can fail** when a still-quarantined
  sidecar refuses to spawn, and each blocked sidecar may need its own "Open Anyway".
  ConvertIA does **not** silently strip the quarantine xattr (a misleading security
  gesture). Instead this is surfaced honestly as the distinct **`QuarantinedByOs`**
  error kind (§2.8) — distinguished from `EngineMissing`/`BundleDamaged` — whose copy
  tells the user to use Privacy & Security → "Open Anyway" and retry (§2.8.2 row).
  **Canonical `QuarantinedByOs` message `[DECIDED]` (plain language, NO stack trace, MUST name
  the specific blocked sidecar):** *"Could not launch {engine name} — blocked by macOS
  security. Open System Settings → Privacy & Security and click \"Open Anyway\" next to
  {engine name}, then try again."* The `{engine name}` is the friendly sidecar name (e.g.
  "FFmpeg", "LibreOffice", "pandoc") so the user knows **which** "Open Anyway" to click; the
  §2.8 catalog owns the string (this is the fixed text it carries). The Sequoia
  final-confirmation step (the OS shows a final "click **Open** to confirm" dialog after
  "Open Anyway") is part of the §6.2.4 step-by-step.
  The §7.2.3 macOS-ordering caveat ensures this surfaces **in a window**, not as a
  silent pre-window hang. The user-facing download-page steps (blocked-on-first-launch
  → Privacy & Security → Open Anyway → sidecars may each need it) are owned by §6.2.4;
  the §6.6 macOS walkthrough must **specifically test+pass this on Sequoia** (the
  unsigned-build usability floor depends on it). The *technical fact* (no
  auto-unquarantine; per-sidecar quarantine is real) is owned here.
  - **`QuarantinedByOs` → retry-flow mapping (per-sidecar, no auto-retry) `[DECIDED]`:**
    when a conversion fails because a sidecar is still quarantined, the item fails with
    `QuarantinedByOs` (§2.8) and **ConvertIA does NOT auto-retry** — there is no watcher
    that re-spawns the sidecar when the user approves it. The recovery loop is: the §2.8
    plain-language message tells the user to approve **that sidecar** in **System Settings
    → Privacy & Security → "Open Anyway"**, then **re-convert** (re-drop / re-pick and
    Convert again). Because each sidecar is quarantined independently, a batch using
    multiple engines (e.g. FFmpeg then pandoc) can surface `QuarantinedByOs` **more than
    once** — once per not-yet-approved sidecar — until every sidecar the user's conversions
    touch has been approved; after a sidecar is approved its quarantine xattr is cleared by
    macOS and a subsequent spawn succeeds (the kind no longer fires for it). v1 does not
    pre-warm or batch-approve sidecars; the §5.2 `QuarantinedByOs` surface is the per-item
    fail row with the approve-then-re-convert guidance.
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
   conversion only). **The canonical v1 strings `[DECIDED]`:**

   | Info.plist key | English usage-description string |
   |----------------|----------------------------------|
   | `NSDesktopFolderUsageDescription` | *"ConvertIA needs access to your Desktop to read files you convert there and save the results next to them. Everything stays on your Mac."* |
   | `NSDocumentsFolderUsageDescription` | *"ConvertIA needs access to your Documents to read files you convert there and save the results next to them. Everything stays on your Mac."* |
   | `NSDownloadsFolderUsageDescription` | *"ConvertIA needs access to your Downloads to read files you convert there and save the results next to them. Everything stays on your Mac."* |
   | `NSRemovableVolumesUsageDescription` | *"ConvertIA needs access to removable drives (USB sticks, SD cards) to read files you convert from them. Everything stays on your Mac."* |

   (Honest-disclosure wording: local conversion only, nothing uploaded — consistent
   with §2.11. These are the §7.2 deliverable; §5 may refine UI-chrome wording, not
   these OS-level strings.)
2. **Do not rely on the responsible-process chain holding for spawned engines.** A
   separately-spawned sidecar engine (our copyleft-isolation model, §3.6) that opens
   a protected path *can* hit a TCC denial in some chain-break edge cases. (Note:
   macOS tracks a **responsible-process chain**, so a child usually *does* inherit
   the parent's grant — the earlier "children never inherit the grant" framing was
   overstated; the mitigation is sound as **defence-in-depth** against the cases
   where the chain breaks, not because inheritance never happens.) **`[REC]`
   Mitigation (READ side): the Rust core (which holds the TCC grant) is the **only**
   process that first *reads* a TCC-protected source — it copies the source into the
   app-owned per-job scratch (§2.14 kind-2, the §3.5.0 macOS source-staging copy) and
   hands the engine the **scratch path**, never a raw protected user path.** Engines
   therefore never *read* from a protected location directly, so a TCC chain-break on
   the read side can never block a conversion. **Scope note (WRITE side) `[DECIDED]`:**
   this absolute is **scoped to READS only**. The §2.14.1 publish temp (`out_tmp/.part`)
   is a sibling dotfile **inside the destination dir**, and on the SSOT-default
   beside-source path that destination dir *is* itself a TCC-protected location
   (Desktop/Documents/Downloads/removable). The core (not the engine) is also the
   process that first *creates* that `.part` and performs the §2.1 exclusive publish,
   so the first write is still core-initiated — but a write into a beside-source
   destination dir **can** still be TCC-gated, and a denial there **fails that item**
   per §2.8 (the `QuarantinedByOs`/unreadable-or-denied kind) while the batch
   continues. There is therefore **no claim that "a TCC chain-break can never block a
   conversion" on the write side** — only that engines never *touch* a protected path
   directly (read via staged scratch, write via the core's publish). This dovetails
   with the §2.14 cross-volume strategy and the §2.12 isolation wrapper. The *engine
   arg/handle plumbing* is owned by **§3.5** (see §3.5's macOS source-staging
   subsection); §7.2 owns the *requirement* that engines never be the process that
   first reads a TCC-protected path, and that beside-source output writes are the
   core's (never the engine's).

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
- **Recorded posture (P2.77) `[DECIDED]`.** The *no-tray*, *no-background/agent*, and
  *closing → quit* legs above are recorded as ONE standing v1 negative — the SSOT
  *portable, no installation, no system pollution* posture — enforced by ABSENCE: the
  v2 `app.trayIcon` config key is absent from `tauri.conf.json`, and no tray / agent /
  login-item API is called anywhere in the codebase (there is no tray or
  background-mode code to register on the Tauri `Builder`). The **P1.16 `window_model`
  scan** structurally asserts the CONFIG side of this — exactly one `main` window,
  `app.trayIcon` absent, and no programmatic window-builder in the production source
  (the broader "no tray/agent API anywhere" is a plain code absence, not a positive
  scan assertion). The same `window_model` module also carries the **§7.2.1-step-6
  `visible: false` reveal-gating assertion** (P2.106.6) — a startup-sequence fact
  (the window ships hidden and the core reveals it only after readiness), distinct
  from this no-tray posture, homed there because both are config-declared window facts. The closing → quit leg is the §7.3.2 `CloseRequested` → §7.3.3
  confirm-guard → `RunEvent::Exit` lifecycle (P2.78–P2.82); the release-time
  no-system-pollution proof is the §6.10 row-21 gate (P10). No box adds a tray, a
  background/agent mode, a login-item, or a persisted window (§7.4) in the v1 line.
- **Window size/position:** see §7.4 — **not** persisted in v1 `[REC]`; the window
  opens at a sensible default size each launch.

### 7.3.2 Lifecycle event wiring (Rust)

Two complementary hooks own the lifecycle (per Tauri v2):

- **`WindowEvent::CloseRequested { api, .. }`** — fires when the user clicks the
  window's close control. Registered via **`Builder::on_window_event(|window, event| …)`**
  (the builder hook). **In Tauri v2 the closure takes TWO arguments** —
  `(window: &Window, event: &WindowEvent)`. (The v1 single-argument
  `&GlobalWindowEvent` form — with `event.window()` / `event.event()` — was removed
  in v2.) ConvertIA inspects *converter state* and may `api.prevent_close()`
  to run the §7.3.3 guard. The v2 frontend equivalent is listening for
  **`tauri://close-requested`** (e.g.
  `getCurrentWindow().listen('tauri://close-requested', …)` /
  `onCloseRequested(e => e.preventDefault())`). **`[REC]` do the decision in Rust**
  (the core owns batch state) and only use the JS side to render the confirm UI
  (§5.2), to avoid a split-brain "is it converting?" check.
- **`RunEvent::ExitRequested { api, .. }`** and **`RunEvent::Exit`** — handled by the
  closure passed to **`App::run`** (i.e. `builder.build(ctx)?.run(|app, event| …)` —
  the run-event handler is on the built `App`, **not** on `Builder`). `ExitRequested`
  is the last chance to `api.prevent_exit()`; `Exit` is the final cleanup point (flush
  logs, best-effort scratch cleanup). **`best_effort_scratch_cleanup` IS the same idempotent
  §2.6 `cleanup_run` path `[DECIDED]`** — not a separate cleanup implementation: it invokes
  the §2.6 run-end cleanup (remove the central `run-<RunId>/` dir + `*.part` in the recorded
  `final_dir` set), **best-effort and non-blocking** (it must not stall app exit). If a
  **wedged descendant** still holds a `.part` at exit (§1.7 group-kill timed out), that temp
  is **deferred to the §2.6.3 next-launch startup sweep** — exit never blocks waiting on it.

```rust
.on_window_event(|window, event| {                 // v2: two args (&Window, &WindowEvent)
    if let WindowEvent::CloseRequested { api, .. } = event {
        if converter_is_busy(window.app_handle()) {   // run-level state (§1.9)
            api.prevent_close();
            // Payload: use serde_json::Value::Null (or a Serialize+Clone unit struct) —
            // NOT the bare `()` literal, which does not serialize reliably across all
            // Tauri v2 versions for emit. §0.4-owned event → §5.2 confirm UI.
            window.emit("app://close-requested", serde_json::Value::Null).ok();
        }
    }
})
// …
// the run-event handler lives on the built App, not Builder:
builder
    .build(tauri::generate_context!())?
    .run(|app, event| match event {
        // §7.3.3 QUIT-leg guard [DECIDED]: busy-gated prevent_exit + the SAME confirm signal as the
        // window-close guard above (macOS app-menu Quit / Cmd+Q raises ExitRequested with NO per-window
        // CloseRequested, so the window-close guard alone under-delivers there). `code: None` only —
        // a PROGRAMMATIC `app.exit(code)` is never blocked (see the §7.3.3 programmatic-exit exemption).
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => {
            if converter_is_busy(app) {
                api.prevent_exit();
                app.emit("app://close-requested", serde_json::Value::Null).ok();
            }
        }
        // Open-with [DECIDED] — RunEvent::Opened is a `#[cfg(any(target_os = "macos",
        // target_os = "ios", target_os = "android"))]` enum VARIANT in Tauri v2: it does
        // NOT exist on Windows/Linux (their intake is argv/single-instance). The `.run()`
        // closure REGISTRATION is unconditional (one funnel), but the Opened match ARM
        // carries the variant's SAME cfg — where the variant is absent it is compiled out
        // (an unconditional arm would NOT compile on Win/Linux). Of ConvertIA's shipped
        // DESKTOP triples (no mobile build) the arm is reachable only on macOS — not a
        // second cross-platform intake path. Distinct from the §7.1.1 argv callback. MUST
        // route through the SAME funnel so the §7.1.1 refuse-busy gate is enforced here too
        // (a mid-conversion Open-with otherwise bypasses the PRIMARY gate — it never goes
        // through the argv callback on macOS). origin = LaunchArg on a first-launch Opened
        // (app not yet ready → buffered), SecondInstance otherwise.
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        RunEvent::Opened { urls } => {
            let paths: Vec<PathBuf> = urls.iter().filter_map(|u| u.to_file_path().ok()).collect();
            let origin = if frontend_ready(app) { IntakeOrigin::SecondInstance } else { IntakeOrigin::LaunchArg };
            forward_launch_intake(app, paths, origin); // → §7.8.1 funnel → busy gate + §1.1
        }
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

**Programmatic-exit exemption `[DECIDED]`:** the §7.3.2 quit-leg guard applies only
to the **user/OS quit request** (`RunEvent::ExitRequested { code: None }` — app-menu
Quit, Cmd+Q, an OS shutdown request). A **programmatic** `app.exit(code)` (which
raises `ExitRequested` with `code: Some(..)`) is **never blocked** — it is exactly
the sanctioned exit step 2's confirmed quit performs after the cancel + cleanup, so
the confirm flow can never deadlock against its own guard. Both guard legs
(window-close `prevent_close`, quit-leg `prevent_exit`) share the ONE §7.3.2 busy
predicate and the ONE `app://close-requested` confirm signal.

### 7.3.4 In-flight queue on close `[DECIDED]`

There is **no persistent queue** and **no resume across launches** (consistent
with §7.4 "persist nothing"): a pending/running queue (§1.9) exists only in memory
for the lifetime of the process. Quitting discards Pending items (they were never
written) and cancels the Running one (§7.3.3). On next launch the user re-drops.
(Resumable batches would require persisting source/target/settings and re-checking
the frozen set — out of v1, parked alongside presets.)

---

## 7.4 Persistence & app state `[DECIDED: v1 persists only a 3-key cosmetic/diagnostic blob]`

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
most** the three purely-cosmetic/convenience/diagnostic values below — never anything
derived from the user's files:

| Key | Type | Default | Why it's defensible |
|-----|------|---------|---------------------|
| `theme` | `"system" \| "light" \| "dark"` | `"system"` | UI preference, not user data; re-asking every launch is annoying. §5.5 owns the theme itself. |
| `lastDestinationMode` | `"beside-source" \| "<absolute path>"` | `"beside-source"` | Re-uses a *chosen* destination (§2.7) across launches; **stores a folder path the user explicitly picked, never a source path or filename**. |
| `verboseLog` | `bool` | `false` | The §7.5.3/§5.9 diagnostic-logging opt-in; persisting it spares the user re-enabling it each session when chasing a bug. A pure on/off flag, no user data. §7.5 owns the logging behaviour. |

**`[DECIDED]`: ship the 3-key blob.** It stays inside "no user data / no history",
improves everyday feel, and is trivially inspectable — adopting the standing [REC].
(Dropping it would cost nothing functionally — theme → always `system`, destination →
always `beside-source`, verbose → always `false` — so this is a low-stakes default, not
a load-bearing call.)
**A `lastDestinationMode` path is always re-validated as writable at use time** (§2.7
per-location fallback applies if it has since become read-only/gone) — it is a
*hint*, never a guarantee. The blob's location/mechanism is §7.4.2; its capability
(`store:default`) is on the §0.10 allowlist.

### 7.4.2 If shipped: where it lives & how `[REC]`

- **Mechanism:** the official **`tauri-plugin-store`** (a single JSON file,
  `settings.json`), or a hand-rolled equivalent — either is fine; the store plugin
  is the lower-effort default. Capability `store:default` (§0.10 owns the allowlist
  entry). **Scope accuracy `[DECIDED]`:** `store:default` grants all store operations
  (`load`/`get`/`set`/`delete`/`save`/…) with **no pre-configured per-file scope** — it
  applies to **every store the plugin creates**, not one file (verified vs the v2 store
  plugin permission reference; there is no Tauri-native per-file store scope). ConvertIA
  achieves **effective single-file scoping only by convention**: it only ever opens/uses
  the store API for `settings.json` (one `Store.load('settings.json')` call site, no other
  store names). This is a code convention, **not** a permission restriction — do not
  describe the capability as "scoped to the one file".
- **Location (per-OS, via Tauri `app.path().app_config_dir()`):**
  - Windows: `%APPDATA%\dev.ne-ia.convertia\settings.json`
  - macOS: `~/Library/Application Support/dev.ne-ia.convertia/settings.json`
  - Linux: `$XDG_CONFIG_HOME/dev.ne-ia.convertia/settings.json` (→ `~/.config/…`)
- **Reconciling with SSOT Principle 2 "portable / no system pollution":** this single
  tiny cosmetic file in the OS-standard **per-user** config dir is **not a registry write,
  not system-wide, not next to the binary, not a service/LaunchAgent/daemon**, holds no user
  data, and is **trivially removable** (delete the one folder). That is the honest reading of
  Principle 2 — a per-user preference file is not "system pollution" (no installer, no
  scattered system state); the §6.10 row 21 Procmon/fsusage/strace gate explicitly permits
  writes to the OS config/log dir and the user's chosen output, and nothing else.
  **Config-dir location `[DECIDED]` (was `[OPEN→README, minor]`):** `settings.json` lives in
  the **OS per-user config dir** (adopting the `[REC]`), **not** beside the executable — a
  beside-binary file breaks when the portable app runs from a read-only medium (USB/DMG),
  and the OS config dir is the cross-platform-correct, writable home regardless of where the
  binary sits.
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

### 7.5.1 Decision: a local, opt-in-verbosity log exists `[DECIDED]`

**`[DECIDED]` ship a local, on-disk log, default level `warn`/`info`,** using
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
  - Linux: Tauri **`app_log_dir()`** → `~/.config/dev.ne-ia.convertia/logs/`
    (Tauri v2 resolves `app_log_dir()` on Linux via the **config** dir —
    `${configDir}/${bundleIdentifier}/logs`, `configDir = $XDG_CONFIG_HOME`, default
    `~/.config`. This deviates from the strict XDG `$XDG_STATE_HOME` (`~/.local/state`)
    where logs would "officially" live — we follow Tauri's `app_log_dir()` for
    cross-platform consistency, not raw XDG)
- **Rotation/retention `[DECIDED]`:** `tauri-plugin-log` `.max_file_size(5_000_000)`
  (bytes) with **`RotationStrategy::KeepOne`** (the bounded-footprint choice). **API fact
  (verified against `tauri-apps/plugins-workspace` `plugins/log/src/lib.rs` on the `v2`
  branch):** `RotationStrategy` has **three** variants — **`KeepAll`**, **`KeepOne`**, and
  **`KeepSome(usize)`** (there is **no** `KeepN`). We choose **`KeepOne`** for a bounded
  single-file footprint; **`KeepSome(n)`** is the available alternative if a small rolling
  history is later wanted. **Footprint bound — re-verified at source `[DECIDED]`:** the
  `KeepOne` rotation arm is literally `fs::remove_file(&self.path)?` — it **deletes** the
  old file (it does **not** rename it to a `.bak`/`.log.old` backup, unlike `KeepAll`/
  `KeepSome`, which call `rename_file_to_dated()`), so on reaching `max_file_size` the
  on-disk maximum is **~1× `max_file_size` (≈5 MB)**, NOT ~2×. (This was re-checked against
  the pinned plugin version's source specifically because a single-file disk bound is
  load-bearing for the "leave nothing behind / no system pollution" budget; if a future
  plugin version ever changes `KeepOne` to rename-to-backup, do **NOT** switch to
  `KeepSome(0)` — that still calls `rename_file_to_dated()` on rotation, briefly
  producing a dated file alongside the new one and so **breaking** the single-file ~1×
  bound. Instead implement a **manual rotate: delete the existing log file before the
  plugin opens the new one** (or vendor a `KeepOne`-delete variant). `KeepSome(0)` does
  **not** preserve the single-file footprint.)
  `KeepOne` keeps the log from ever silently growing (consistent with "leave nothing
  behind" and "no system pollution"). The concrete crate version is pinned in the lockfile
  + SBOM per the §0.8 no-hardcoded-digits policy. **Audit trail `[DECIDED]`:** the
  `KeepOne == fs::remove_file` (≈1× `max_file_size`) source-verification above is recorded
  against the **exact pinned `tauri-plugin-log` version/commit in the lockfile** — so the
  ~1×-footprint claim is auditable, and a version bump triggers a re-check of the `KeepOne`
  rotation arm before the bound is re-asserted (`[DEFER: verify-on-bump]`). **Concrete audit
  (P2.92):** the source-verification above was re-run against the lockfile-pinned
  **`tauri-plugin-log` 2.8.0** — confirmed the `KeepOne` arm is `fs::remove_file(&self.path)?`
  (a delete, not `rename_file_to_dated()`) and `max_file_size` is a `u128` byte cap, so the
  ~1× single-file bound (≈5 MB) holds for this pin; the standing verify-on-bump trigger above
  re-runs it on the next version bump.

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
- **Verbose / "diagnostic" mode (off by default) `[DECIDED]`:** an explicit user
  opt-in (a toggle reachable from About §5.9, or an env var / `--verbose` launch
  flag) that *additionally* records **full paths** and the **exact engine command
  line** (§3.5) for reproduction. Turning it on shows a one-line notice that the
  log will now include file paths and is **still purely local** (nothing is sent).
  This is the deliberate, disclosed trade: privacy by default, full reproducibility
  on demand — and it never changes the no-network property.
  - **Effect timing `[DECIDED — read-at-startup, effective next launch]`:**
    ConvertIA **resolves the verbose level once at startup** (the app's setup stage): the
    `verboseLog` prefs key (§7.4) is read via `prefs::load` — the log plugin is registered on the
    Builder before any AppHandle exists, so the persisted pref is applied in setup, not at the
    plugin's own init — OR-combined with the `--verbose` flag and applied via `log::set_max_level`
    (a `convertia_core`-scoped `Debug` `level_for` ceiling keeps verbose output to ConvertIA's own
    records); flipping the About §5.9
    toggle persists the new value but **takes effect on the next launch**, consistent with
    the env-var / launch-flag framing. The About toggle therefore shows an **"applies after
    restart"** note (§5.9) so the user is never misled that mid-session logging changed.
- **No automatic upload, ever.** The §6.8 `SECURITY`/bug-report flow asks the user
  to *attach* the log file **manually** to a report; ConvertIA neither reads it
  back nor transmits it. "Phone home" stays impossible (§2.11).
- **Recorded stance (P2.96) `[DECIDED]`.** The *no automatic upload, ever* leg above is
  recorded as ONE standing v1 negative — the SSOT *Local, private & offline* posture (§2.11)
  applied to the diagnostic log — enforced **structurally, by absence**, not by policy:
  ConvertIA's core opens no socket for any log or bug-report path (there is no upload /
  transmit / "send report" code anywhere — the §2.11.1 **T9a** half), the WebView cannot
  originate the request either (the §0.10 CSP `connect-src 'self' ipc:` + the capabilities
  allowlist grant it no HTTP/fetch surface, §2.11.1), and no telemetry or crash-reporter
  transmits (§2.11.2 — the local log is local-only and never sent). The §6.8 `SECURITY.md`
  bug-report flow is therefore the user **manually attaching** a default-redacted (§7.5.3)
  log to a private advisory; the frontend-error JS-bridge (P2.95) and verbose mode
  (P2.94/P2.94.1) only *write* to that same local file, neither adds a transmit path. The
  observable proof is the §2.11.4/§6.7.3 offline-egress gate — zero outbound packets across
  a full conversion (P9); no box in the v1 line adds an upload, auto-attach, or transmit path.

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
- **Recorded stance (P2.97) `[DECIDED]`.** Bullets 1–3 above are recorded as ONE standing
  v1 negative — the SSOT *Local, private & offline* "does not check for updates or phone
  home" posture applied to startup — enforced **structurally, by absence** (bullet 1's
  missing updater dependency / endpoint / manifest / pubkey) behind the §0.10 CSP and the
  ungranted HTTP/updater capabilities (bullet 3), not by policy. The **only** version
  signal is the user-initiated About→Releases link (§7.6.2), a §7.7 shell-out ConvertIA
  never fetches or parses itself. The observable proof is the §2.11.4/§6.7.3 offline-egress
  gate run with zero-startup-network in the same window (a §6.5/§2.11 release gate, P9); the
  §7.2.2 runtime zero-startup-network assertion is the P2.107 leg. A future opt-in check
  stays parked, not present in v1 (§7.6.3).

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

- **Recorded stance (P2.99) `[DECIDED]`.** The parked `updateCheckOptIn` is recorded as ONE standing
  v1 negative — the SSOT "any future update check would be opt-in and disclosed, never silent" posture —
  enforced **structurally, by absence**: the §7.4.1 persistence blob is a CLOSED 3-key set (`theme` /
  `lastDestinationMode` / `verboseLog`), so no `updateCheckOptIn` key exists to read or toggle in v1 (a
  4th key would break the §7.4.1 closed-3-key decision). It is the persisted-state companion to the P2.97
  no-startup/background version-check stance (§7.6.1) and the §2.11.2 no-phone-home invariant: v1 has
  **no** update-check surface at all, opt-in or otherwise. Adding the key later stays gated on the SSOT
  opt-in/disclosed/visible bar — a deliberate future decision, not a v1 gap.

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
successor to the old `shell.open` allowlist). **`[DECIDED]` the WebView does NOT call
the opener plugin directly** — the three operations are ConvertIA's **own** typed IPC
commands (C9/C10, §0.4.1), and their **Rust handlers call the plugin's `OpenerExt`
methods internally**. A Rust-internal `OpenerExt` call is **not** capability-gated
(capabilities gate only what the WebView may invoke), so the §0.10 manifest carries
**no `opener:*` grant** at all. Three operations (canonical command names/payloads
enumerated by §0.4 — **C9 takes an `OpenTarget` id, never a path** (the 2026-07-06
owner ruling, core-owned paths); its Rust handler **resolves the id against the run's
`RunResultStore`** — the core-side registry holding the terminal run's recorded real
paths (§1.12) — then calls the `OpenerExt` method on the resolved path):

Every `OpenTarget` variant maps to a concrete resolution + `OpenerExt` call (the C9
`target` argument selects the row; no `OpenTarget` is undefined):

| C9 `OpenTarget` / C10 | Resolves to (`RunResultStore`, §1.12) | `OpenerExt` method (Rust, called internally) | Used by |
|-----------|-----------|-----------|---------|
| **`OpenTarget::CommonRoot`** | the run's recorded `common_root` (the beside-source root, §2.7) | folder browse: `app.opener().reveal_item_in_dir(..)` (JS API name `revealItemInDir`) where a single subject output exists to highlight — Explorer `/select,` on Windows, Finder reveal on macOS — else `open_path(dir, None)` (JS API name `openPath`) on the root itself; Linux falls back to the plain folder-open (no reliable cross-distro select). **The primary "Open folder" affordance** (§5.3 OpenActions) — safer than a file launch (nothing is executed, §7.7.3). | C9 → "Open folder" / "Open source folder" (§5.3) |
| **`OpenTarget::DivertRoot`** | the run's recorded `divert_root` (present only for a split-output batch, §1.12) | the same folder-browse call on the divert root (typically the many-outputs case → `open_path(dir, None)`) | C9 → "Open saved-to folder" (§5.3) |
| **`OpenTarget::Item(ItemId)`** | that item's recorded OUTPUT file | `app.opener().open_path(path, None)` (JS API name `openPath`) — opens the converted **file** in the OS default app for its type (single-output "Open file") | C9 → "Open file" (§5.3) |
| **`OpenTarget::Residue(ItemId)`** | that item's recorded cleanup-residue location (§2.6.4) | `app.opener().reveal_item_in_dir(residue)` — reveal only, never a launch | C9 → the "reveal residue" link (§5.3 ResultSummary) |
| **(C10, no `OpenTarget`)** | — (no resolution; the URL is a compiled-in constant) | `app.opener().open_url(URL, None)` (JS API name `openUrl`) — opens the canonical Ne-IA URL in the default browser | C10 → About link (§5.9 / §7.6.2) |

**"Open folder" target (per SSOT *How It Feels* 8):** `OpenTarget::CommonRoot`
resolves to the **common root of the dropped selection** (the mapping is owned by
§1.12/§2.7); for the beside-source default that is the dropped folder, for a
chosen-destination it is that destination root. On Windows/macOS the reveal API
additionally highlights the specific output when a single file is the subject; Linux
file managers vary, so the **`[REC]`** fallback is "open the containing directory"
(no reliable cross-distro select).

**Split-output (divert) → TWO open-folder targets `[DECIDED]`.** When a batch's
outputs split between beside-source and a divert root, the `RunResultStore` records
**both** roots and the wire `RunResult` mirrors them as **`commonRootDisplay`** +
**`divertRootDisplay?`** (§0.6/§1.12); the §7.7.3 resolution set covers both. The §5.3
`OpenActions` therefore renders **two open-folder buttons** in that case — "Open
[beside-source]" → C9 `{ target: OpenTarget::CommonRoot }` and "Open
[Downloads/Documents]" → C9 `{ target: OpenTarget::DivertRoot }`; when
`divertRootDisplay` is absent it renders only the single common-root button. A single
root target could not represent both roots, which is exactly why the model splits
them — a one-button Summary would leave a returning-to-Downloads user with no
navigation to the diverted half.

### 7.7.2 Where the gate lives (no static opener scope) `[DECIDED]`

Because the WebView calls **only** ConvertIA's C9/C10 commands — never the opener
plugin directly — the opener-path gate is **not** a static capability scope (§0.10
carries no `opener:*` grant). This is deliberate and necessary: the §2.7 default writes
output **beside the source** (Desktop, USB, arbitrary project folders), routinely
**outside** any OS-known root like `$DOWNLOAD`/`$DOCUMENT`, so a static glob scope could
**never** cover the common case — it would only silently break the open-folder/open-file
DoD gate. The **real, sufficient gate is Rust-side**:

- **Open-target resolution (C9):** the WebView **cannot name a path at all** — C9's
  argument is an **`OpenTarget` id** (the 2026-07-06 owner ruling, core-owned paths),
  and the Rust handler **resolves it against the current run's `RunResultStore`**
  (recorded outputs + roots + residue locations, §1.12/§7.7.3) **before** calling
  `OpenerExt` — works for *arbitrary* beside-source destinations, not just OS roots.
  An id that does not resolve is refused (and logged, §7.5): an unknown `ItemId`, no
  terminal run, `DivertRoot` on an undiverted run, `Residue` on a residue-free item.
  Membership is **total by construction** — an id can only ever denote something the
  run recorded — so the pre-revision "validate the WebView-supplied path against the
  recorded set" check, and its compare-two-paths anti-TOCTOU/canonicalization
  questions, dissolve: no WebView path exists to validate, canonicalize or race; the
  only path in play is the core's own recorded one. A membership rule like this is
  exactly what a static glob could never express. (When outputs split between
  beside-source and a divert root, **both** roots are open-folder targets and **both**
  resolve — a single `common_root` could not represent the diverted half, §0.6.)
- **Open-url (C10):** the command takes **no URL argument** from the WebView; the Rust
  handler opens a **compiled-in canonical Ne-IA URL constant**, eliminating any
  URL-injection surface. (A capability allow-list, even if present, can only
  further-restrict an outer bound applied *before* the handler — it could never widen
  to cover beside-source outputs, which is exactly why the load-bearing check is the
  Rust-side `RunResultStore` id-resolution, not a manifest scope.)

### 7.7.3 Open-file safety `[DECIDED + REC]`

Launching an **external** application on a **fresh, possibly-untrusted** artifact
is security-relevant (§0.11 maps this threat to §7.7). Constraints:

- **Per-FILE launch vs per-ROOT folder-browse — two distinct resolution rules
  `[DECIDED]`.** The "never a source" claim is scoped to the **file-launch** target,
  not the folder-browse targets:
  - **File launch** (`OpenTarget::Item(ItemId)` — hand the artifact to its OS default
    app): the id resolves **only into the current `RunResultStore`'s recorded OUTPUT
    files** (§1.12) — never an arbitrary WebView-named path (none exists on this wire,
    2026-07-06 owner ruling), never a *source* file, never an engine intermediate
    (neither is in the resolvable output set). An id that does not resolve is refused
    (and logged, §7.5). This makes *file launch* structurally unable to
    **execute/hand-off** anything other than a file the user just chose to create.
  - **Folder browse** (`OpenTarget::CommonRoot` / `OpenTarget::DivertRoot` /
    `OpenTarget::Residue(ItemId)` — open/reveal in the OS file manager): these resolve
    to the run's **roots** — `common_root` (beside-source) and, for a split-output
    batch, `divert_root` (§1.12/§7.7.3) — or to the item's recorded cleanup-residue
    location (§2.6.4). **Opening a root is intentionally allowed even though, on a
    beside-source batch, `common_root` is a directory that *contains the user's
    sources*** — this is a folder *browse*, **no handler is executed on any file**
    (the OS just shows the directory, with an output highlighted where the reveal API
    supports it), so it cannot launch a source. The resolution rule therefore admits a
    *root / residue location* for the browse targets and an *output file* for the
    file-launch target; it does **not** treat the source-containing root as forbidden.
    "Never a source" means **no source file is ever handed to a default app**, not
    "the folder holding sources is never browsable".
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
- **Recorded stance (P2.105) `[DECIDED]`.** The open-file-safety constraints above are recorded
  as ONE standing v1 posture — SSOT *How It Feels to Use* (one-click open-folder) balanced against
  the §0.11 threat of launching an external app on a fresh, possibly-untrusted artifact (mapped to
  §7.7): **(a) no auto-open** (explicit "Open file" click only, §5.3 — never at end-of-batch),
  **(b) reveal-in-folder is the preferred/primary affordance `[REC]`** ("Open file" secondary),
  and **(c) the OS picks the handler** (ConvertIA chooses no program except the C10 browser-for-URL
  case, §7.7.1). Enforced by the §7.7.2/§7.7.3 Rust-side `RunResult` membership gate (C9,
  P2.100–103, unit-tested G15 — file-launch admits only a recorded OUTPUT file, folder-browse only
  a run ROOT); the no-auto-open behaviour + the affordance ordering are exercised by the §6.4.6 E2E
  / §6.6 walkthrough. It is the open-side companion to the §7.6.1 no-phone-home stance: v1 launches
  nothing the user did not just create and did not explicitly click.
  - **Supersede-note `[the 2026-07-06 owner ruling (core-owned paths)]`.** The
    P2.105-recorded gate above was formulated as a **path-membership** test (a
    WebView-supplied C9 `path` validated against the run's recorded set — built
    P2.100–103, G15-unit-tested). The ruling re-cuts the C9 wire to **`OpenTarget` ids
    resolved against the `RunResultStore`** (§7.7.1/§7.7.2): identical force — only
    what the run recorded can be opened, legs (a)–(c) unchanged — in a simpler form
    (no WebView path exists, so the validate/canonicalize/compare surface is gone).
    The P3 wire-revision boxes own the code change; the P2.100–103 mapping + gate
    LOGIC carry over as the membership core of the id resolution.

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
  app), surfaced by Tauri v2 as **`RunEvent::Opened { urls: Vec<Url> }`** handled in the
  `App::run(...)` closure — this is the **sole** macOS file-open mechanism for ConvertIA.
  **NOT `tauri-plugin-deep-link` / `on_open_url` `[DECIDED]`:** that plugin handles
  **custom-scheme deep links** (`myapp://…`), a *different* OS intent; it does **not** fire
  for the Open-With / double-click open-documents AppleEvent that delivers `file://` URLs,
  so using it for file intake would silently never trigger. ConvertIA registers **no** URL
  scheme (§7.8.2 negative), so `on_open_url` is irrelevant here. The payload is
  **`Vec<Url>` (`file://` URLs), not paths** — each is converted via
  `Url::to_file_path()` before §1.1. This is the **launch/open-with** hook, **distinct**
  from the single-instance second-launch hand-off (§7.1.1, the `argv`/cwd callback); the
  single-instance plugin (§7.1.1) ensures both land in the one instance. **The
  `RunEvent::Opened` handler routes through the same `forward_launch_intake` funnel
  (below), so the §7.1.1 refuse-busy gate is enforced on a mid-conversion Open-with too**
  — a macOS Open-with against a running, busy app is refused (paths dropped), not merged
  into the frozen set (§2.4). Without this, Open-with would bypass the PRIMARY gate on
  macOS (it never goes through the argv callback there). **`RunEvent::Opened` is a cfg-gated Tauri-v2 VARIANT (API fact) `[DECIDED]`:**
  in Tauri v2 `RunEvent::Opened` is a **`#[cfg(any(target_os = "macos", target_os = "ios",
  target_os = "android"))]` enum variant** — it does **not exist** on Windows or Linux. Of
  ConvertIA's shipped **desktop** triples (macOS/Windows/Linux; no mobile build) it is
  therefore reachable **only on macOS**, so **Win/Linux intake correctness rests entirely
  on the argv / single-instance path**, never on `Opened`. The `App::run` **closure
  registration is unconditional** (one funnel, no `cfg` around the `.run(...)` call), but
  the **`RunEvent::Opened` match ARM carries the variant's same `cfg`** (the full
  `target_os` triple above): an unconditional arm would **fail to compile** on
  Win/Linux (the variant is absent), so where the variant is absent the arm is compiled out
  — a no-op for Open-with rather than a second intake path. (There is **no** "may also fire
  cross-platform" claim — that would be a wrong Tauri-v2 API fact.)
- **Windows:** files passed as **`argv`** to the process. Captured in the
  single-instance callback (§7.1.1) for a second launch, and read at first launch
  in `setup` (`std::env::args_os`).
- **Linux:** the desktop-entry **`%F`/`%U`** field expansion → `argv`, handled the
  same as Windows.
- **Native window drop (all platforms) `[DECIDED — the 2026-07-06 owner ruling
  (core-owned paths)]`:** the §5.4 drop — **`WindowEvent::DragDrop`** on the main
  window, handled **core-side** (the WebView renders drag-over styling from DOM drag
  events only; the dropped paths never enter it). Not a *launch* entry point, but it
  **joins this same funnel** (origin `Drop`), so the refuse-busy gate, the
  `PendingIntake` stash and the `app://intake` nudge are ONE mechanism for every
  intake source — drop, launch-arg, second-instance, Open-with, and the C2a picker's
  picked set (§0.4.1 C2a).

```rust
// One funnel for EVERY intake source → the core-side PendingIntake buffer. The
// single-instance argv/cwd callback (§7.1.1), the macOS RunEvent::Opened handler
// (§7.3.2), the native window drop (WindowEvent::DragDrop, §5.4) and the C2a picker
// all call THIS, so the refuse-busy gate (§7.1.1), the stash and the nudge are
// enforced once, at the single funnel — not duplicated per hook.
fn forward_launch_intake(app: &AppHandle, paths: Vec<PathBuf>, origin: IntakeOrigin) {
    if paths.is_empty() { return; }
    // §7.1.1 PRIMARY refuse-busy gate, enforced HERE for ALL intake hooks: a
    // mid-conversion hand-off / drop must NOT reach §1.1 (it would violate the §2.4
    // freeze). When busy: DROP the paths — no stash, no nudge. (No new app:// event:
    // the §0.4.2 three-event invariant holds. The §5.8 BusyNotice Banner is the
    // defence-in-depth surface if a nudge ever leaks mid-run; the primary gate's job
    // is to never let it leak — incl. via Opened and the drop.)
    if converter_is_busy(app) {                  // run-level state (§1.9), same predicate as §7.3.2
        return;
    }
    // Not busy: ALWAYS stash. PendingIntake is the single hand-off buffer for every
    // source; the real PathBufs + the IntakeOrigin stay HERE, core-side — no path
    // ever crosses the wire or an app:// event (the 2026-07-06 owner ruling). A
    // stash over a still-undrained set APPENDS to it and keeps the FIRST stash's
    // origin (the P2.58 no-loss accumulation — a superseding replace would drop
    // the earlier launch's paths; prose below). ORDER MATTERS
    // (no-loss): stash BEFORE reading the ready flag — paired with the drain's
    // mark-ready-BEFORE-take, every stash is either taken by a drain or sees
    // ready == true and nudges (prose below).
    stash_pending_intake(app, paths, origin);
    // Nudge iff the frontend is ready: `app://intake` is PAYLOAD-LESS — a pure
    // "come and drain" signal (§0.4.2). Not-ready (first launch): no nudge required —
    // the root-shell mount ALWAYS drains once (§5.8), which collects this stash.
    if frontend_ready(app) {
        app.emit("app://intake", serde_json::Value::Null).ok();
    }
}

// The native drop enters the SAME funnel (§5.4): the WebView renders drag-over
// styling from DOM drag events only and never sees the dropped paths.
.on_window_event(|window, event| {
    if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
        forward_launch_intake(window.app_handle(), paths.clone(), IntakeOrigin::Drop);
    }
    // (the §7.3.2 CloseRequested arm lives in this same handler)
})

// Caller at the single-instance hook (§7.1.1) resolves argv → paths, origin = SecondInstance;
// the first-launch setup reader uses origin = LaunchArg; the macOS RunEvent::Opened handler
// converts urls → paths and uses LaunchArg (first launch) / SecondInstance (already running).
// parse_path_args classification rules [DECIDED]: argv[0] is skipped; a `-`-leading token is
// a launch switch (never a path); an EMPTY token is DROPPED (never cwd-joined — a join would
// make the launching directory itself an ingest root); a Windows drive-relative token
// (`C:file.txt`) passes through unchanged and resolves against the per-drive cwd at freeze
// time, where a missing file fails clearly (§2.8).
fn forward_launch_argv(app: &AppHandle, argv: &[String], cwd: &str, origin: IntakeOrigin) {
    forward_launch_intake(app, parse_path_args(argv, cwd), origin);
}
```

**`PendingIntake` — the single hand-off buffer; `app://intake` — a payload-less nudge
`[DECIDED — the 2026-07-06 owner ruling (core-owned paths)]`.** Every non-busy intake —
drop, launch-arg, second-instance, Open-with, the C2a-picked set — lands in the managed
`State<PendingIntake>` (real `PathBuf`s + the stored `IntakeOrigin`, held core-side; no
path ever crosses the wire), and `app://intake` carries **nothing** — a pure "come and
drain" signal. The consumption is **C1 `drain_intake { collectingId, onScan } →
CollectedSet`** (§0.4.1): the handler **consumes `PendingIntake` exactly once per call**
using the stored `origin` (the real one — a first-launch buffered set drains as
`LaunchArg`, never a hard-coded `SecondInstance`), freezes the buffered set (§1.1/§2.4)
and returns its `CollectedSet`; a drain that finds **nothing pending** returns
`CollectedSet::Empty` and the UI stays put — a clean no-op (the ordinary no-files mount
drain, or a nudge whose stash a concurrent drain already consumed). The frontend issues
the drain **on every `app://intake` nudge and once on root-shell mount** — the mount
drain fires only after the `app://intake` listener registration has **settled**
(completion, not merely call order) and collects a first-launch / Open-with set that was
buffered before any listener existed (the documented Tauri first-frame timing pitfall a
bare emit would lose); the buffered set returns via the C1 command **response**, never
via an event, so nothing depends on the listener for the drain itself. The drain
therefore runs **multiple times per session** (once per nudge + once on mount);
**consume-once per CALL still holds**. **No-loss ordering — two rules replace the former
lock dance:** the funnel **stashes before it reads the ready flag**, and the drain
**marks ready before it takes** — so every stash either precedes a take (consumed by
that drain) or follows the mark (its ready-read sees `true` and emits the nudge that
triggers its own drain); a nudge that races an already-run drain merely produces a
harmless empty drain. A stash over a still-undrained set **APPENDS** to it and keeps the
FIRST stash's `origin` (the P2.58 no-loss accumulation — a superseding replace would
silently drop the earlier launch's paths, the exact loss this section's guarantee
forbids); the next drain consumes the merged set. There is **no
separate `take_pending_intake` command/accessor**, so the canonical C1–C13 IPC table
(§0.4.1) stays complete and the codegen/drift check covers the whole drain path; **no
4th `app://` event** is added (the §0.4.2 three-event invariant holds).

> **Supersede-note `[the 2026-07-06 owner ruling (core-owned paths)]`.** The
> pre-revision funnel split **Emit-vs-Buffer** — emit `app://intake { paths, origin }`
> when frontend-ready, else buffer into `PendingIntake` — and closed the first-launch
> listener race with the P2.137 `stash_or_route`/`RouteToEmit` no-loss re-route (the
> fused `take_marking_ready` + the stash's under-lock ready re-check, recorded
> 2026-07-06). Both **collapse into stash+nudge**: with no payload-carrying emit arm
> there is nothing to hand back for a "live emit", so `RouteToEmit` and the
> pending-slot lock dance are structurally unnecessary — the residual interleavings
> are covered by the two-rule no-loss ordering above, and the worst outcome of any
> race is a harmless empty drain, never a stranded or double-ingested set. The former
> drain-once-per-mount framing (a monotonic ready flag guarding a single drain) widens
> to **drain-per-nudge + drain-on-mount** (consume-once per call unchanged; the
> `drainPending: true` C1 flag is retired with the `ingest_paths` shape — every
> `drain_intake` call IS a drain, §0.4.1). The P3 wire-revision boxes own the code
> change.

**Interaction with single-instance + freeze (§7.1.1 / §2.4):** at first launch the
paths seed the idle state as if dropped (via the stash + mount-drain above). A *second*
launch's paths arrive via the single-instance callback; if the primary instance is
mid-conversion, the §7.1.1 `[REC]` (refuse with "busy") applies — the new paths are
**not** silently merged into the frozen set of a running batch (that would violate §2.4
"files appearing after the freeze are never ingested"). When idle, they start a fresh drop.

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

> These are now **resolved** (recorded in the README open-questions log); kept here as a
> trace of where each was decided, not as open calls.
- **§7.1.1** — second-launch hand-off while a batch is **running**: `[DECIDED]`
  **refuse-busy** (UI surface = the `BusyNotice` Banner, §5.3). Owner: §7.1.
- **§7.2.3** — engine integrity: `[DECIDED]` **hash-on-first-launch + cheap warm-launch
  check**, with the concrete `engine-integrity.json` marker (config dir, keyed on
  `app_version`) above. Owner: §7.2 with §3.3.
- **§7.4.1** — persist the minimal 3-key prefs blob (theme +
  last-destination-mode + verboseLog) vs strict zero-persistence. `[DECIDED]` ship the blob.
  Owner: §7.4.
- **§7.4.2** — prefs file in OS config dir vs beside-binary (portability reading).
  `[DECIDED]` **OS config dir**. Owner: §7.4 (minor).
- **§7.5.1/§7.5.3** — ship a local log at all, and the verbose-mode opt-in for
  full-path/command-line capture. `[DECIDED]` **yes to both**, privacy-by-default,
  verbose effect = next launch (§7.5.3). Owner: §7.5.
