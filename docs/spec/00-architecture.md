# 00 — Architecture

> System architecture and the technical skeleton everything else hangs off.
> Origin: SSOT *Portable, no installation*, *Cross-platform, one product*,
> *Local, private & offline*, *Security posture*. **Read together with
> [07-app-shell](07-app-shell.md)** — the process model here depends on its
> instance/run-identity model (§7.1).
>
> **What this file OWNS (authoritative):** the §0.4 IPC contract (the one canonical
> enumeration of commands + events + payloads + error shape + cancellation token),
> the §0.6 domain model (Rust types + invariants), the §0.7 logical-module
> decomposition + physical tree, the §0.8 tech stack & pinned versions, the §0.9
> concurrency degree + engine-subprocess pool, the §0.10 capabilities/CSP
> allowlist, and the §0.11 assembled threat map. **What it REFERENCES (does not
> restate):** the pipeline (§01 owns it; §0.5 is only a map), per-format engine
> detail (04-formats), the guarantees (§02), decoder isolation (§2.12), the
> patent matrix (§3.4), and the app-shell decisions (§07).

---

## 0.1 Goals & constraints recap (from SSOT)

The architecture is the smallest design that simultaneously honours every
load-bearing SSOT promise. These constraints are quoted here only to anchor the
decisions in the rest of the file; their *implementations* live in the owning
sections.

| SSOT promise (name) | Architectural consequence | Owner of the mechanism |
|---|---|---|
| *Portable, no installation* | Single self-contained artifact per OS; no installer, no admin rights, no system services; all engines ride inside the bundle | §0.2, §0.7, §06 |
| *Cross-platform, one product* | One codebase → three builds; identical UX/guarantees; per-platform variance confined to the WebView runtime (§0.3.1) and the §3.4 patent gaps | §0.2, §0.3.1, §06 |
| *Local, private & offline* | Zero network capability in the security boundary: no remote origins in CSP, no `http`/updater plugins on the allowlist; the only network is the user-initiated open-project-page shell-out | §0.10, §0.11, §2.11, §7.6 |
| *Never harm the original* / atomicity | A reusable **guarantees-fs** layer is a first-class module that ALL output flows through; engines never write the final file directly | §0.7, §2.1/§2.3/§2.6/§2.7/§2.14 |
| *Fail clearly* | A single error taxonomy crosses the IPC boundary as one typed error shape (§0.4); panics are caught at the worker boundary (§2.13) | §0.4, §2.8, §2.13 |
| *Security posture* (untrusted decoders) | Decoders run as **separate invoked subprocesses** behind the §2.12 isolation wrapper, never linked into the core; the WebView half is locked by §0.10 | §0.3, §0.9, §0.10, §0.11, §2.12 |
| *It just works by default* | The IPC surface is verb-oriented and stateful in Rust; the frontend is a thin view (§0.3) that never needs to enumerate a directory or hold engine knowledge | §0.3, §0.4 |

`[DECIDED]` These are inherited from Phase 1 and the SSOT; this file does not
re-open them.

---

## 0.2 Framework choice — Tauri `[DECIDED]`

**Decision (Phase 1, honoured here): Tauri v2.** Rust core + a React 19 / TypeScript
/ Tailwind / Vite WebView UI. Engines are **bundled sidecars/resources**, fully
offline (§3.3).

**Why Tauri over Electron / Wails:**

- **Size & portability.** Tauri uses the OS's **system WebView** (no bundled
  Chromium), so the *app shell* is a few MB of Rust + assets rather than ~150 MB
  of browser. This directly serves *Portable, no installation* and offsets the
  one accepted cost of *bundle-everything* — the heavy part of ConvertIA's
  download is the **engines** (FFmpeg, LibreOffice; §3.9), not the framework. An
  Electron baseline would add the browser weight *on top of* the engines.
- **Rust core.** The guarantees (atomic write, resolved-identity link safety,
  frozen set, cleanup) are filesystem- and concurrency-critical and benefit from
  Rust's ownership model and mature crates (see §0.8); subprocess orchestration of
  untrusted decoders wants a strong process/IO story. Electron's main process is
  Node (looser FS/concurrency story); Wails (Go) is viable but the platform's
  existing stack is React/TS, and Tauri lets us **reuse that stack verbatim** for
  the UI.
- **Security model.** Tauri v2 ships an explicit **capabilities/permissions**
  system + CSP (§0.10) — exactly the WebView-side lockdown the SSOT *Security
  posture* and *offline* promises need, declaratively, rather than hand-rolled.
- **Stack reuse.** React 19 / TS / Tailwind / Vite is the Ne-IA platform standard;
  the UI is "just a web app" with a typed IPC seam.

**What Tauri commits us to (trade-offs, addressed in the owning sections):**

1. **WebView runtime variance per OS** — the single biggest portability risk;
   owned by §0.3.1.
2. **Native file-drop, not HTML5 DnD** — the WebView cannot see real FS paths;
   intake is Rust-side (§0.4 boundary fact, §1.1, §5.4).
3. **The Rust↔TS boundary must be typed** to satisfy the platform "no `any`" rule
   — owned by §0.4.5.
4. **Sidecar invocation is gated by the capability allowlist** — the
   shell/`externalBin` scope (§0.10) is the seam through which §3.5 launches
   engines.
5. **The Tauri updater plugin must be deliberately absent** (no phone-home) —
   §7.6.

---

## 0.3 High-level architecture

**Two-tier, three-process-class model.**

```
┌───────────────────────────────────────────────────────────────────────┐
│  ConvertIA process (single instance — §7.1)                            │
│                                                                         │
│  ┌─────────────────────────┐        Tauri IPC (custom protocol)        │
│  │  WebView (UI tier)       │  ◄──────── commands (req/resp) ──────────┐│
│  │  React 19 / TS / Tailwind│  ──────────  events / Channel<T> ───────►││
│  │  • renders state         │                                          ││
│  │  • NO fs, NO engines,    │     ┌────────────────────────────────┐   ││
│  │    NO directory walk     │     │  Rust core (logic tier)        │   ││
│  └─────────────────────────┘     │  • IPC handlers (§0.4)         │   ││
│                                   │  • orchestrator (queue, §1.9)  │   ││
│                                   │  • detection (§1.2)            │   ││
│                                   │  • guarantees-fs (§2.x)        │   ││
│                                   │  • engine-registry seam (§3.2) │   ││
│                                   │  • subprocess pool (§0.9)      │   ││
│                                   └───────────────┬────────────────┘   ││
│                                                   │ spawn (isolated,   ││
│                                                   │  §2.12 wrapper)     ││
└───────────────────────────────────────────────────┼────────────────────┘
                                                     ▼
        ┌──────────────────────────────────────────────────────────────┐
        │  Engine subprocesses (separate invoked binaries — §3.5/§3.6)   │
        │  FFmpeg/ffprobe · LibreOffice (soffice --headless) · poppler    │
        │  pdftotext · pandoc · convertia-imgworker (libvips image-worker │
        │  process — §0.9/§3.5.5, a packaged externalBin)                 │
        │  (Ghostscript [DECIDED: dropped v1] — §3.1)                      │
        │  Untrusted bytes are parsed HERE, never in the core.            │
        └──────────────────────────────────────────────────────────────┘
```

**Tier responsibilities:**

- **WebView (UI tier)** — *view only.* Renders the screen states (§5.2), captures
  user intent, calls commands and subscribes to events/Channels (§5.8). It holds
  **no** filesystem access, **no** engine knowledge, and cannot enumerate a
  directory (the WebView has no real FS paths — §0.4 boundary fact). It is treated
  as **untrusted** by the core (the §0.10 capability allowlist is the contract).

- **Rust core (logic tier)** — *all logic.* IPC handlers (§0.4), the conversion
  orchestrator (queue/lifecycle — §1.9, owned by §01), content detection (§1.2),
  the **guarantees-fs** layer (§0.7; the reusable home of §2.1/§2.3/§2.6/§2.7/
  §2.14), the engine-registry seam (§3.2), and the subprocess pool (§0.9). It is
  the only tier that touches the filesystem and the only tier that spawns engines.

- **Engine subprocesses** — *the actual conversions and the untrusted-byte parsing.*
  Each is a **separate, independently-invoked binary** (aggregation, not linking;
  §3.6) so the MIT core stays clean and a decoder crash/hang is contained.
  Spawned and governed by the §0.9 pool, launched through the §3.5 argument
  construction, **routed through the §2.12 isolation wrapper** (the owner of the
  per-platform isolation mechanism). This section states *that* decoders are
  isolated subprocesses; **§2.12 owns *how* they are isolated** and is referenced,
  not restated, here.

**Process count.** One ConvertIA process (the Tauri host, which embeds the WebView
as the OS provides it) + N short-lived engine subprocesses, where N is bounded by
the §0.9 concurrency degree. No background services, no tray daemon (§7.3).

### 0.3.1 WebView runtime variance & supported-OS floor `[DECIDED — floor below]`

Tauri renders the UI in the **OS-provided WebView**, which differs per platform.
This is the principal portability risk and it interacts hard with *no-network*
(we may not download a runtime) and *no-installation*.

| OS | WebView runtime | Risk | Disposition |
|----|-----------------|------|-------------|
| Windows | **WebView2** (Chromium/Edge) | May be **absent or old** on older Windows; the standard Tauri remedy is the WebView2 **bootstrapper/installer**, but *that downloads at install time* — forbidden by *no-network / no-installation* | **Recommend: rely on the OS, require a present WebView2; do NOT download a runtime in v1.** Windows 11 ships WebView2 by default; Windows 10 has shipped it via Edge/Windows Update for years. **Honest failure mode `[DECIDED]`:** when WebView2 is **absent**, the WebView2 loader fails **before the Rust core runs** — the window flashes and closes and the core **cannot** present a §2.13/§7.2 in-app fault (tauri#12030; there is no built-in detection hook on the portable path). So the "fail clearly" substitute for the **canonical portable artifact (§6.1.2)** is a **§6.2.4 download-page WebView2 prerequisite note**, **not** a runtime dialog — the unconditional "never a silent blank window" promise does **not** hold for the portable launch. `bundle.windows.minimumWebview2Version` is **installer-only** (NSIS/WiX bootstrapper) — it is **inert for the portable artifact**, and since **NSIS is NOT shipped in v1** (§6.1.2 `[DECIDED-6.1a]` — the portable `.zip` is the only v1 Windows artifact) this floor-enforcement mechanism is **not present in v1 at all**. On the portable path the practical floor is the §0.3.1 supported-OS floor (Win10 1809+ ships a recent-enough Evergreen runtime), surfaced honestly via the §6.2.4 download-page prerequisite note. (Stronger options recorded, not v1: a future post-v1 **NSIS per-user installer with bootstrapper** could enforce/install the floor, and/or **bundle a fixed-version WebView2 runtime beside the exe** — a bundled runtime is not a runtime *download*, so no-network holds, at an artifact-size cost.) |
| macOS | **WKWebView** (system Safari/WebKit) | Tied to the OS version; no separate install | Pinned by `bundle.macOS.minimumSystemVersion`. |
| Linux | **WebKitGTK** (`libwebkit2gtk-4.1`) | **Distro drift** — version varies widely; the portable AppImage must carry/locate a compatible WebKitGTK | Bundled/located by the AppImage packaging (§6.1); a missing/incompatible WebKitGTK is a §7.2 startup fault with a plain message. |

**Supported-OS floor (v1) `[DECIDED]`** — adopting the recommended floor (the exact
build numbers stay tunable against the §6.4 drift matrix, but the floor is fixed):

- **Windows 10 (1809 / build 17763) and Windows 11**, x86-64, with WebView2
  present (Evergreen). `minimumWebview2Version` ≈ a recent-but-not-bleeding-edge
  Chromium (e.g. the `110.x` class) so our CSS/JS baseline is safe.
- **macOS 11 Big Sur and later** (covers the WKWebView feature set React 19 + our
  Tailwind build target need; `minimumSystemVersion: "11.0"`). Universal binary
  (Intel + Apple Silicon).
- **Linux: a glibc desktop with `libwebkit2gtk-4.1`** (Ubuntu 22.04 LTS-class and
  newer, Fedora current); shipped as an x86-64 AppImage. ARM is out of v1.
- **Minimum RAM (all platforms) `[DECIDED design; DEFER: corpus-calibrated]`:** **2 GB
  minimum-supported, 4 GB recommended.** The app runs in ≤ 2 GB by bounding concurrency
  (only ≤ the §0.9 degree of items decoded at once, lightweight frozen-set/queue records,
  a virtualized UI list — §1.10) and by degrading gracefully under memory pressure (the
  §1.10 low-memory policy). Below 2 GB it still launches + converts (serially, slower) but
  is outside the tested envelope; the exact floor is calibrated against the §6 corpus.

Status: the **floor is `[DECIDED]`** (Windows 10 1809+/11; macOS 11+; Ubuntu
22.04-LTS-class `libwebkit2gtk-4.1`; x86-64). The architecture is indifferent to the
exact numbers and the *shape* (rely-on-OS WebView, fail clearly at startup if
absent/old, floor declared in config) was always `[DECIDED]`. The only residual is
**[DEFER: validate the precise build numbers against the §6.4 rendering-drift matrix
and §6.1 packaging]** — a calibration detail, not an open commitment.

**Rendering-drift implication (→ §6.4):** because three different browser engines
render the same UI, visual/behaviour drift (CSS, font rendering, drag-events) is a
test concern, not a runtime one. **Startup-time WebView faults (→ §7.2 / §2.13):**
an absent/old/broken WebView is an *app-level* fault, surfaced once, plainly.

---

## 0.4 Frontend ↔ backend boundary (IPC) — **single authoritative contract**

This section is **the** canonical enumeration of the IPC surface. §01 (pipeline)
and §5.8 (UI async model) **reference** these names and shapes; they never restate
or redefine them. The contract is the spine: changing a command/event/payload here
ripples to §01, §05, §0.4.5 codegen, and §06's drift check.

### 0.4.0 Mechanics (Tauri v2 primitives used)

- **Commands** = `#[tauri::command] async fn` handlers, registered in the
  `invoke_handler`, called from TS via `invoke('cmd_name', args)`. Long-running
  work is `async` so the WebView stays responsive (SSOT *visible progress, stays
  responsive*).
- **Shared state** = injected via `State<'_, T>` (e.g. the orchestrator handle,
  the run registry). Commands are thin; they delegate to the orchestrator.
- **One-way streaming Rust→TS** = **`tauri::ipc::Channel<T>`** — the v2 ordered,
  high-throughput channel. **Per-run progress uses a Channel** handed to the
  `start_conversion` command (ordered delivery, backpressure-friendly, scoped to
  the run — preferred over global events for hot per-item progress).
- **Broadcast / app-wide notifications Rust→TS** = `app.emit(event, payload)` /
  TS `listen(event, cb)` — used for **lifecycle-level** events not tied to a
  single run channel (e.g. `auth`-style app faults, startup readiness). The bulk
  of conversion telemetry goes through the Channel, not global events, to avoid
  cross-run leakage.
- **Error shape** = every command returns `Result<T, IpcError>` where `IpcError`
  is a `serde`-serialised enum (§0.4 error shape below). No command ever panics
  across the boundary: the **convert** loop is caught at the per-item worker boundary
  (§2.13.2) and the **intake/detection** path (C1 `ingest_paths` / C2a
  `pick_for_intake` — the §1.1 walk + §1.2 detection, the first code to touch untrusted
  bytes) is caught at its own per-path + whole-walk `catch_unwind` boundary (§2.13.2
  "Intake/detection panic boundary"); both surface as a calm `IpcError`/failure outcome,
  never a blank window.
- **Cancellation** = a process-wide cancellation primitive keyed by `RunId`
  (§0.4 cancellation token). The mechanism that actually kills an in-flight engine
  is owned by **§1.7** (process-group kill; Windows has no SIGTERM); this section
  defines only the *token and the command* that trips it.

**Boundary fact — native file-drop `[DECIDED]`.** In a Tauri WebView, HTML5
drag-and-drop does **not** expose real filesystem paths. Intake therefore uses
**Tauri's native file-drop event** (the window `onDragDropEvent` / `DragDrop`
payload carries real `PathBuf`s) and the native **dialog** picker; **folder
recursion runs in Rust** (§1.1), because the WebView cannot enumerate a directory.
This constrains §1.1 (intake) and §5.4 (DnD UI). The frontend's DnD handler exists
only to drive hover/visual affordance; the *paths* arrive over the native event,
not the DOM drop.

### 0.4.1 Command enumeration (authoritative)

All payloads are the §0.6 domain types (or thin DTOs of them); field naming is
`camelCase` on the wire (Rust `#[serde(rename_all = "camelCase")]`). Pseudo-Rust
signatures; the TS side is generated (§0.4.5 codegen).

| # | Command | Request | Response | Notes |
|---|---------|---------|----------|-------|
| C1 | `ingest_paths` | `{ paths: Vec<PathBuf>, origin: IntakeOrigin, collectingId: CollectingId, drainPending?: bool, onScan?: Channel<ScanProgress> }` | `CollectedSet` | Builds the **frozen source set** (§2.4): recurse folders (Rust), ignore hidden/system files, de-dup by resolved identity (§2.3), run detection (§1.2), group by user-facing format (§1.3). Returns the collected-summary (detected format + count) **or** a `MixedDrop` / `Unsupported` / `Uncertain` outcome. `origin` distinguishes drop / picker / launch-arg (§7.8). The frontend generates `collectingId` and passes it in so C13 can cancel this in-flight walk **before** C1's long await resolves (see note). **`drainPending` (first-launch drain) `[DECIDED]`:** the frontend cannot hold the buffered launch paths (they live in the Rust-side `State<PendingIntake>`, §7.8.1), so the first-launch drain is a C1 call with **`paths: []` + `drainPending: true`**: the handler, seeing the flag, **consumes `PendingIntake`** (using its stored `origin`, typically `LaunchArg`) and freezes THAT set, returning its `CollectedSet`; if `PendingIntake` is empty it returns `CollectedSet::Empty`. A normal intake call omits `drainPending` (or `false`) and uses its `paths`. The two are mutually exclusive (a `drainPending: true` call ignores any `paths`). | **Optional `onScan` Channel `[DECIDED]`:** carries a **throttled live scan count** (`ScanProgress { scanned: u32 }`, ~2/s, §0.6) so the §5.2 *Collecting* state can show "Scanning… N files so far" during a long recursive walk; it is a **run-telemetry-style Channel**, NOT one of the three `app://` events (the §0.4.2 "no other IPC events" invariant covers `app.emit` events, not Channels handed to a command). |
| C2a | `pick_for_intake` | `{ kind: PickKind /* files \| folder */, collectingId: CollectingId, onScan?: Channel<ScanProgress> }` | `CollectedSet` | The **intake picker `[DECIDED]`.** Opens the native files/folder dialog **Rust-side via `DialogExt`** from this command's handler (so there is no `dialog:allow-open` WebView grant — §0.10). The picked paths are funnelled **straight into the C1 `ingest_paths` freeze Rust-side** and this command returns the **same `CollectedSet`** C1 returns — so **no raw FS path ever reaches the WebView** (the WebView only triggers the picker and receives the collected summary, never paths to re-submit). A **cancelled dialog is a clean no-op** that returns `CollectedSet::Empty` with no error and leaves the UI in Idle (§5.4). Takes the same `collectingId` + optional `onScan` as C1 so C13 can cancel the in-flight walk. |
| C2b | `pick_destination` | `{}` | `Option<PathBuf>` | The **destination-folder picker `[DECIDED]`.** Opens the native folder dialog **Rust-side via `DialogExt`** (still no `dialog:allow-open` grant) and **returns the chosen folder `PathBuf` to the WebView**, which carries it into **C5 `set_destination`** (and then C6). **This one path DOES transit the WebView** — unavoidable, because the destination is a WebView-held choice (§5.10 "Change destination") — and is **acceptable**: it is a *write* destination, not a source path, bounded by the §2.1 non-destructive creates (a chosen destination can never harm an original or read anything; §0.11 T2). `None` = the user cancelled (no-op; the held C4/C5 destination is unchanged). The "picked paths never transit the WebView" claim is scoped to the **intake** picker (C2a) only. |
| C3 | `get_targets` | `{ collectedSetId: CollectedSetId }` | `TargetOffer` | From the detected source type → the offered `Vec<Target>` + the **one pre-highlighted default** + per-target lossy flags + per-target availability (from §3.4) + the declared options model (§1.6). Pure function of detection; no engine spawned. |
| C4 | `plan_output` | `{ collectedSetId, target: TargetId, options: OptionValues, destination: DestinationChoice }` | `OutputPlanPreview` | Computes the `OutputPlan` (§1.8): resolved destination, beside-source vs chosen-root subtree re-creation, per-location divert preview, **re-run/equivalent-output detection (§2.5)** → may return a `RerunPrompt`. Also returns the §1.10 pre-flight verdict (size/space estimate, any up-front "too big" fail). Drives the "will save to …" line (SSOT *output lands somewhere obvious*) **before** convert. |
| C5 | `set_destination` | `{ collectedSetId, target: TargetId, options: OptionValues, destination: DestinationChoice }` | `DestinationResolved` | User changes the destination before convert; revalidates writability/divert **and re-evaluates the destination-dependent preflight** — the §2.14.4 free-space check on the new volume — returning a refreshed `PreflightVerdict` so the UI's held C4 verdict never goes stale (§1.8 destination-change re-validation). The §2.5 re-run verdict is **destination-INDEPENDENT in v1** (EquivKey has no destination component, §2.5.1) and is **carried through unchanged** from C4 — C5 does **not** recompute `rerun`. |
| C6 | `start_conversion` | `{ collectedSetId, target, options, destination, rerunDecision: RerunDecision, onProgress: Channel<ConversionEvent> }` | `RunId` | Creates a `RunId`, enqueues the batch (§1.9), spawns workers (§0.9), and **streams `ConversionEvent`s over the Channel** (E-series below). Returns immediately with the `RunId` (the run proceeds async; the Channel carries all telemetry). **C6's `destination` argument is AUTHORITATIVE `[DECIDED]`:** C4/C5 are plan/preview + revalidation only — there is **no separate server-side destination store**; the value the UI passes to C6 is what the run uses (the UI carries the last C5-resolved destination into C6). |
| C7 | `cancel_run` | `{ runId: RunId }` | `()` | Trips the §0.4 cancellation token for that run. The actual in-flight engine kill is §1.7's mechanism. Already-finished items are kept (SSOT *cancellable*); the in-progress item is discarded cleanly (§2.1/§2.6). |
| C8 | `get_run_summary` | `{ runId: RunId }` | `RunResult` | The end-of-batch summary (§1.12): per-item success/fail/skip + reasons + output→source map + residue warnings (§2.6). Also delivered as the terminal `ConversionEvent::RunFinished`; this command is the idempotent re-fetch (e.g. after a WebView reload). |
| C9 | `open_path` | `{ kind: OpenKind /* folder | file | revealInFolder */, path: PathBuf }` | `()` | The DoD "one-click open-folder/open-file" action. The Rust handler **validates `path` against the current `RunResult`'s recorded outputs (or their common root)** (§7.7.3 — the real, sufficient gate; works for arbitrary beside-source destinations) and then calls the opener plugin's `OpenerExt` (reveal/open) **internally**. **How** it shells out per OS is owned by §7.7; **which** path is allowed is the §7.7.3 RunResult check; there is **no `opener:*` WebView capability** (§0.10). |
| C10 | `open_project_page` | `{}` | `()` | The **only** permitted network action — user-initiated open of the canonical GitHub project/releases URL in the default browser (SSOT *Local/private/offline* "only network activity is user-initiated"). The Rust handler opens a **fixed URL constant** via `OpenerExt::open_url` internally; the WebView supplies no URL, so this single origin is the only reachable one (§7.6). No `opener:*` WebView grant (§0.10). |
| C11 | `get_app_info` | `{}` | `AppInfo` | Version, build id, and the **third-party-licenses / NOTICE** data for the About screen (data generated by §3.7; displayed by §5.9). No network. |
| C12 | `get_engine_health` | `{}` | `EngineHealth` | Startup self-check result: which bundled engines are present/runnable, which §3.4 patent-gated targets are available on this platform. Feeds §5.2 (disable/omit unavailable targets) and §7.2 (startup faults). Cached from the §7.2 startup probe; cheap to call. |
| C13 | `cancel_ingest` | `{ collectingId: CollectingId }` | `()` | Cancels an **in-flight** `ingest_paths` (C1) — the recursive walk/detection of a thousands-file folder (§1.10) can run long enough that the §5.2 *Collecting* state's cancel-collect control must have a backing command. Trips an **ingest-scoped `CancellationToken`** keyed by the pre-`RunId` `CollectingId` (§0.6) that the **frontend generated and passed to C1** (see note) — so C13 can name the in-flight walk even though C1's own response hasn't returned yet. The §1.1 walkdir/detection loop polls it and stops cooperatively, discarding the partial (un-frozen) set — **no cleanup obligation** (no temp is written during ingest). Keyboard: §5.10. |

**Notes binding to other owners:**

- `ingest_paths` is the single freeze point (§2.4) for **all** intake origins
  (drop, picker, launch args / second-instance hand-off — §7.1/§7.8).
- **Ingest cancellation handle `[DECIDED]`.** So C13 `cancel_ingest` can target an
  in-flight ingest (a drop's **C1** *or* the intake picker's **C2a**, which funnels
  through the same C1 freeze), the **frontend generates the `CollectingId` and passes it
  as a C1/C2a argument** (the single-funnel option). The Rust core registers the
  ingest-scoped `CancellationToken` under that id at handler entry (for **C2a**, *before*
  the native dialog opens — §1.1, so a C13 during the modal is honoured; for **C1**, at the
  start of the walk), trips it on C13, and **drops it on EVERY handler exit branch** — the
  normal walk-completes return, the C13-tripped return, **and** the C2a cancelled-dialog →
  `CollectedSet::Empty` return (the walk loop that normally drops it never runs there, so
  the handler drops it explicitly — no token leak). This mirrors the §0.4.4 `RunId` token
  lifecycle, one phase earlier.
  This keeps a single freeze point **and** keeps the §0.4.2 "no other IPC events"
  invariant true — there is **no** `collecting-started` event (an earlier draft
  proposed emitting one; rejected so the event enumeration stays closed).
- `get_targets`/`plan_output`/`start_conversion` together realise the SSOT flow
  *drop → pick target → (see destination) → convert*; the **pipeline that runs
  inside `start_conversion` is owned entirely by §01** — this contract only fixes
  the boundary.
- There is intentionally **no per-item-target command** — the **one-Target-per-
  Batch** invariant (§0.6) is enforced by the shape of `start_conversion` (a
  single `target` for the whole `collectedSetId`).
- **C4 vs C5 — byte-identical payloads, different contract `[DECIDED]`.** C4
  `plan_output` and C5 `set_destination` take the **same** request fields, but **only C4
  computes `rerun`** (the §2.5 equivalence check) and the §1.10 **`preflight` verdict**;
  **C5 never recomputes `rerun`** (it carries the C4 `rerun` through unchanged — the v1
  EquivKey is destination-independent, §2.5.1) and re-evaluates only the
  destination-volume `preflight`. Because the signatures alone cannot distinguish them,
  the orchestrator **enforces the asymmetry by lifecycle, NOT by a one-shot rule**:
  - **C4 is callable at any point in state 4 `[DECIDED]`.** It is called **eagerly on the
    `3→4` (target-chosen) transition with the pre-highlighted default already selected**,
    then **re-callable (debounced ~150 ms, §5.8) on any target or option change** so the
    "will save to …" line, divert preview, `rerun`, and `preflight.up_front_fail` verdict
    never go stale. There is **no "fires exactly once"** constraint — the multi-call
    behaviour §5.8 requires is canonical (an orchestrator that rejected the re-calls would
    break the Targets/options UI).
  - **C4 never overrides a C5 destination `[DECIDED]`.** Destination authority lives with
    C5: once the user has changed the destination (a C5 on a given `collectedSetId`), a
    **subsequent C4 on that same collected-set must carry the C5-resolved destination in its
    `destination: DestinationChoice` argument** — C4 never resets the destination to a
    different value. A post-C5 target/option change (the realistic §5.2 rows 4/5 flow:
    enter-Targets → change-destination → reconsider-and-change-target) is therefore **legal
    and still re-runs C4** (debounced, §5.8) so `rerun`, `preflight.up_front_fail`, the lossy
    note and the "will save to …" line never go stale — but the orchestrator **feeds the
    held C5 destination back into the recomputed plan** (the caller passes it, or the
    orchestrator re-applies the retained C5 destination if C4 arrives carrying a stale
    default). The bound is narrow: **C4 may re-plan freely post-C5; it just cannot change
    the destination away from the C5 value.** Further *destination* changes still go through
    C5 only. This is the ONLY ordering rule.

  So "C4 computes `rerun` + `preflight`, C5 never recomputes `rerun`" is an enforced
  orchestrator rule (computed values, not just prose); the destination-independent
  EquivKey is the §2.5 [DECIDED] this rests on.

### 0.4.2 Event / Channel enumeration (authoritative)

**Run telemetry — `Channel<ConversionEvent>`** (handed to `start_conversion`,
C6). A `#[serde(tag = "type", content = "data")]` enum, ordered delivery:

| Variant | Payload | Meaning |
|---|---|---|
| `RunStarted` | `{ runId, totalItems, willReencode: bool }` | Batch accepted; queue built. **`totalItems` = QUEUED (eligible) items only** (= `CollectedSet::Single.count`, i.e. `CollectedSet::Single.items.len()` — `members` is the INTERNAL §1.3 `Grouping::Single` field, never on the §0.6 wire), **excluding pre-flight-skipped items** (§1.1/§1.3 — they never enter the queue); it is the `BatchProgress.total` denominator, so a skipped item never holds the bar below 100% (skips reconciled only at the §1.12 Summary) `[DECIDED]`. `willReencode` is a **conservative source-container → target-pair worst-case** flag (**re-encode *possible* ⇒ `true`**), **NOT a header/inner-codec inspection** — `RunStarted` is emitted right after C6, **before any `ffprobe`** (§1.7/§1.10 defer `ffprobe` to convert-time), so the inner codecs of MKV/MOV are **unknown** at emission and the flag is decided purely from the (source-container, target) pair (§2.9.2): `true` ⇒ at least one item *may* re-encode → video shows the worst-case lossy note ("may be re-encoded"). A pair whose only possible path is remux-verbatim is `false`; any pair that *could* re-encode is `true`. **Emission rule `[DECIDED]`:** for non-video / non-applicable batches the core emits **`willReencode: false`** (never omitted) so the field always carries a definite value. **The Rust struct field is non-optional `bool` (line below), so the GENERATED `bindings.ts` type is non-optional `willReencode: boolean`** — there is no third `undefined` state. (Hand-written docs/comments elsewhere sometimes show `willReencode?` purely as a decode-tolerance convenience — consumers still treat any absent/`undefined` as `false`, §5.8 — but the generated binding is non-optional.) The exact per-item disposition is resolved at convert-time (§3.5); the summary (§1.12) reflects the actual outcome. |
| `ItemStarted` | `{ runId, itemId, sourcePath, target }` | An item left `Pending` for `Running` (§1.9). |
| `ItemProgress` | `{ runId, itemId, fraction: Option<f32> /* 0.0..1.0; None only where truly indeterminate (LibreOffice, §1.11) */, stage: JobStage }` | **Real per-item progress** (SSOT *not an indeterminate spinner*). Denominator is engine-specific (e.g. video = source duration from `ffprobe`, §3.5/video.md). `stage` is the §0.6/§1.11 `JobStage` (`Spawning \| Decoding \| Encoding \| Writing`); for the `None`-fraction LibreOffice case the frontend synthesises a staged determinate-looking bar from `stage` transitions (§1.11/§5.3). |
| `ItemFinished` | `{ runId, itemId, outcome: ItemOutcome }` | Terminal per item: `Succeeded { outputPath } \| Failed { error: IpcError } \| Skipped { reason } \| Cancelled`. **Pre-flight-skip emission policy `[DECIDED]`:** pre-flight-skipped items (§1.1/§1.3 — never entered the queue, §1.9) are **NOT** emitted as live `ItemFinished{Skipped}` Channel events; they appear **only** in the terminal `RunFinished → RunResult.items` projection (§1.12). The `ItemOutcome::Skipped` variant is **reserved for that terminal-projection path** (it is not dead wire code — it carries the projected pre-flight skips and any mid-run cooperative skip), so the orchestrator emits **no live `ItemStarted`/`ItemFinished{Skipped}`** for a freeze-time skip; the ProgressList shows skipped rows only once the run reaches `Summary`. (Chosen over a post-`RunStarted` batch flush: pre-flight skips have no queue presence and no per-item work, so surfacing them once, terminally, is simpler and matches §1.9's "never enter the queue".) |
| `BatchProgress` | `{ runId, done, total }` | Aggregate queue progress for the batch bar (§1.11). **Denominator = QUEUED (eligible) items only `[DECIDED]`:** `total` counts only items that entered the queue (= `CollectedSet::Single.count`, i.e. `CollectedSet::Single.items.len()` — NOT the internal §1.3 `members`), **excluding** pre-flight-skipped items (§1.1/§1.3 — they never enter the queue, emit no live `ItemStarted`/`ItemFinished`, and §1.11's numerator excludes them). If `total` counted dropped-but-skipped items the bar could never reach 100%. Skips are reconciled **only** at the §1.12 Summary ("N converted, M skipped"). `total == RunStarted.totalItems`. |
| `RunFinished` | `RunResult` | Terminal for the run; mirrors C8. Carries the full summary incl. residue warnings (§2.6). |

**The complete enum + payload structs (the concrete type `collect_events![]` (§0.4.5)
needs).** All derive `Clone, Serialize, specta::Type` (no `any`; in `collect_types!`):

```rust
#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum ConversionEvent {
    RunStarted(RunStarted),
    ItemStarted(ItemStarted),
    ItemProgress(ItemProgress),
    ItemFinished(ItemFinished),
    BatchProgress(BatchProgress),
    RunFinished(RunResult),          // §0.6 RunResult (mirrors C8)
}

#[derive(Clone, Serialize, specta::Type)] #[serde(rename_all = "camelCase")]
pub struct RunStarted   { pub run_id: RunId, pub total_items: u32, pub will_reencode: bool }
// total_items = QUEUED (eligible) items only (= CollectedSet::Single.count, i.e.
// CollectedSet::Single.items.len() — `members` is the INTERNAL §1.3 Grouping::Single field,
// never serialised on the §0.6 wire type), NOT including pre-flight-skipped items (§1.1/§1.3,
// which never enter the queue). This is the BatchProgress.total denominator; skips reconciled
// only at the §1.12 Summary. [DECIDED]
#[derive(Clone, Serialize, specta::Type)] #[serde(rename_all = "camelCase")]
pub struct ItemStarted  { pub run_id: RunId, pub item_id: ItemId, pub source_path: PathBuf, pub target: TargetId }
#[derive(Clone, Serialize, specta::Type)] #[serde(rename_all = "camelCase")]
pub struct ItemProgress { pub run_id: RunId, pub item_id: ItemId, pub fraction: Option<f32>, pub stage: JobStage }
#[derive(Clone, Serialize, specta::Type)] #[serde(rename_all = "camelCase")]
pub struct ItemFinished { pub run_id: RunId, pub item_id: ItemId, pub outcome: ItemOutcome }
#[derive(Clone, Serialize, specta::Type)] #[serde(rename_all = "camelCase")]
pub struct BatchProgress{ pub run_id: RunId, pub done: u32, pub total: u32 }
```
(`will_reencode` is a plain `bool` on the wire — the core always emits a definite value,
§2.9.2 emission rule; `JobStage`/`ItemOutcome`/`RunResult` are the §0.6 types.)

> **Why a Channel, not events, for run telemetry:** ordering (progress monotonic
> per item), throughput (a 5000-file batch emits a lot), and **scoping** (the
> Channel dies with the run — no cross-run leakage, no global listener cleanup
> bug). This is the Tauri v2 recommended pattern for streamed Rust→frontend data.

**Intake scan telemetry — `Channel<ScanProgress>`** (optional, handed to `ingest_paths`,
C1). Same Channel pattern as run telemetry (NOT an `app://` event):

| Variant / payload | Meaning |
|---|---|
| `ScanProgress { scanned: u32 }` | A **throttled** live count (≈2/s, coalesced) of files seen so far during the §1.1 recursive walk + §1.2 detection, so the §5.2 *Collecting* state can show "Scanning… N files so far". Best-effort, monotonic, dies with the C1 call. |

**App-wide events — `app.emit` / TS `listen`** (not run-scoped):

| Event | Payload | Meaning |
|---|---|---|
| `app://fault` | `AppFault` | An **app-level** fault (§2.13): WebView core disconnect, a startup engine-missing escalation, damaged bundle. The UI shows a plain, no-stack-trace message (§5.8 backend-disconnect handling). |
| `app://intake` | `{ paths, origin }` | The OS handed the running (single) instance new paths via a **second-instance launch / Open-with** (§7.1/§7.8), **and the app was IDLE**. **IDLE-path only `[DECIDED]`:** the refuse-busy check is **core-side** in `forward_launch_intake` (§7.8.1) **before** the freeze — while a run is in flight the core **refuses-busy and DROPS the paths core-side**, so it does **NOT** emit `app://intake` with ingestable paths mid-run (the only mid-run UI surface is `BusyNotice`, §5.3, driven by window re-focus, not this event). On the idle path the core emits `app://intake` and the frontend reacts by calling C1 `ingest_paths`. `origin` is only ever `LaunchArg` / `SecondInstance` here (drop & picker go via C1/C2a directly, never through this event — so a frontend `app://intake` handler needs no `Drop`/`Picker` branch). Cross-ref §1.1. |
| `app://close-requested` | `()` | The OS window-close was intercepted **while a run is in flight** (§7.3.2): the core called `prevent_close` and asks the frontend to show the quit-while-converting confirm (§5.2/§7.3.3). The emit/intercept mechanism is owned by §7.3; the event name is fixed here. |

Apart from these three (`app://fault`, `app://intake`, `app://close-requested`),
there are **no other IPC events**. No telemetry, no heartbeat, no network-driven
event — consistent with *offline / no phone-home* (§2.11, §7.6).

### 0.4.3 Error shape (authoritative) — `IpcError`

Every command's `Err` and every `ItemOutcome::Failed.error` is one shape:

```rust
#[derive(Serialize, specta::Type)]   // generated into bindings.ts; in collect_types![] (§2.8)
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    /// Stable machine code from the §2.8 taxonomy — drives UI branching + i18n.
    pub kind: ErrorKind,
    /// Pre-localised plain-language English message (the §2.8 catalog string).
    /// NEVER a stack trace, never raw engine stderr (SSOT *no stack traces*).
    pub message: String,
    /// Optional path the error concerns (for the summary's output→source map).
    pub path: Option<PathBuf>,
    /// Optional residue location when cleanup could not complete (§2.6) — so the
    /// item is never reported as a clean success.
    pub residue: Option<PathBuf>,
}

#[derive(Serialize, specta::Type)]   // generated into bindings.ts; in collect_types![] (§2.8)
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    // Wire mirror of §2.8 `ConversionErrorKind` — names are byte-identical to the
    // owner (§06 drift check enforces this). Item-level (§2.8):
    Corrupt, Empty, Unrecognized, UnsupportedType, UnsupportedPair,
    Unreadable, Gone, PasswordProtected, NoAudioTrack, TooBig, OutOfDisk,
    WriteFailed, PathTooLong, TooManyCollisions, EngineCrash, EngineHang, EngineError,
    PlatformUnavailable, QuarantinedByOs, CleanupResidue, InternalError,
    // run/app-level (§2.13); surfaced via app://fault:
    EngineMissing, WebviewFault, BundleDamaged,
    // pre-flight (NOT carried as an IpcError; mirror-only for drift-lock — see note below).
    //   MixedDrop has NO §2.13 producer: it is the CollectedSet::Mixed SUCCESS return from C1
    //   (§0.6), driving the §5.2 MixedDropRefusal state 9. It lives here ONLY so the wire enum
    //   stays byte-identical to the §2.8 catalog — do NOT search §2.13 for its producer.
    MixedDrop,
}
```

> **Note — `Cancelled` is not an `ErrorKind`.** A cancelled item is the
> `ItemOutcome::Cancelled` variant (§0.4.2), not a failure; it never carries an
> `IpcError`. The wire enum mirrors **only** the §2.8 taxonomy.
>
> **Note — `MixedDrop` is never carried as an `IpcError`.** Like `Cancelled`, **no code path
> produces `Err(IpcError { kind: MixedDrop })`** — a mixed drop is returned as the **success**
> value `CollectedSet::Mixed { found }` from C1 (§0.6), which drives the §5.2 `MixedDropRefusal`
> state 9. The `MixedDrop` `ErrorKind` entry exists **only as the byte-identical wire mirror**
> of the §2.8 catalog (so the enum stays drift-locked), **not** as a producible run/app-level
> error — do **not** search §2.13 for a `MixedDrop` producer (there is none); its producer is
> the `CollectedSet::Mixed` success-return.

- **Both `IpcError` and `ErrorKind` derive `specta::Type` and are registered in
  `collect_types![]`** (consistent with §2.8 §2.8.2): tauri-specta generates
  `bindings.ts` only from `specta::Type` types, so without the derive `ItemOutcome::
  Failed.error` and every command `Err` would generate as `any` — a no-`any`-rule
  violation. The §06 bindings-drift check (§0.4.5) covers both.
- The **authoritative enumeration of failure kinds and their exact English
  strings is owned by §2.8** (the message catalog). `ErrorKind` here is the wire
  mirror; §06 includes a drift check that the §2.8 catalog and this enum stay in
  lock-step. **The concrete anti-drift mechanism is owned by §2.8.2 `[DECIDED]`:**
  preferably **`ErrorKind` is a `type` alias for the §2.8 `ConversionErrorKind`** (one
  enum, nothing to drift); if a distinct wire type is needed, a `static_assertions`
  variant-count check + a variant-name round-trip `#[test]` make a missing mirror a
  **compile/test failure**, with the §06 codegen-drift diff as the third backstop. `message`
  is filled from the §2.8 catalog **in Rust** (strings live
  with their owner; the UI does not assemble outcome strings — §5.7).
- `kind` is the stable contract the UI branches on (e.g. `PasswordProtected` →
  "password-protected" copy; `EngineMissing` → app-fault screen).

### 0.4.4 Cancellation token (authoritative)

- A `RunId` indexes a `CancellationToken` (recommend `tokio_util::sync::
  CancellationToken`) held in the run registry (`State`). `cancel_run` (C7)
  calls `.cancel()`.
- Workers poll/await the token at safe points and, crucially, the **§1.7
  invocation layer** wires the token to the engine subprocess so a cancel triggers
  the process-group kill (§1.7 owns the kill mechanism and the ordering that keeps
  §2.6 cleanup and §2.1 no-partial intact). This section owns only the token's
  *identity and lifecycle* (created in C6, tripped by C7, dropped on `RunFinished`).
- Cancellation is **cooperative at the orchestrator level, forceful at the engine
  level** (kill the child), reconciled by §1.7.

**Run-registry retention (so C8 can re-serve after a WebView reload).** The run
registry retains the terminal **`RunResult` in memory** (process-local, no on-disk
persistence — consistent with §7.4) **until a new run starts or the app exits**, so
**C8 `get_run_summary` can idempotently re-serve** the summary after a WebView
reload (the exact case C8 names). "The cancellation token is dropped on
`RunFinished`" (above) drops only the *token*, **not** the `RunResult` — the result
outlives the token for re-fetch.

> **Reload-during-run is NOT a supported recovery path on macOS in v1 `[DECIDED]`.**
> There is a **known still-open macOS Tauri crash when the WebView reloads while an async
> command / `invoke` is in flight** (tauri-apps/tauri #9933 / #12338 — distinct from the
> #12030 WebView2-absent case the spec already cites). So C8's "idempotent re-serve" and
> the long-lived `Channel<ConversionEvent>` cover a **FRESH listener attaching after the
> run has already terminated** (re-fetch the retained `RunResult`, re-subscribe for a new
> run) — **not** a reload *mid-stream* while C6's run is still emitting. v1 does **not**
> claim reload-during-run resilience on macOS; the §6.4.6/§6.6 macOS verification covers
> the post-terminal re-serve, and §5.8 surfaces a mid-run IPC drop as `AppFault` (the run
> path), never as a silently-recovered reload. (Windows/Linux are not affected by this
> specific bug, but v1 scopes the guarantee to post-terminal re-serve uniformly.)

**Collected-set registry (so C3/C4/C5/C6 can resolve a `CollectedSetId`) `[DECIDED]`.**
C3 `get_targets`, C4 `plan_output`, C5 `set_destination` and C6 `start_conversion`
each take only a `collectedSetId` and must resolve it to the **frozen `CollectedSet`**
(detected format, frozen `items`, dropped `roots`, `skipped`) — C3 reads the stored
source format, C4/C5 plan against the stored roots, C6 rebuilds the `Batch` from the
stored frozen items (§2.7 needs the roots for subtree re-creation). The core therefore
holds a **collected-set registry**: a `State<'_, T>` map **`CollectedSetId →
FrozenCollectedSet`** (the `CollectedSet::Single` payload + its `roots`), mirroring the
`RunId`-token / `CollectingId`-token lifecycle pattern. **Lifecycle:** an entry is
**created when C1/C2a returns** a `CollectedSet::Single` (the freeze, §2.4), **retained
through C3/C4/C5/C6**, and **evicted** when its run starts (C6 hands the frozen items to
the `Batch`), or when a new C1/C2a supersedes it, or at app exit. C3 is thus a **pure
function of the stored detection result** and C6 builds the `Batch` from the **stored
frozen items** — no second walk, no re-detection. (`Mixed`/`Unsupported`/`Uncertain`/
`Empty` outcomes are terminal pre-flight states and are **not** registered — only a
`Single` yields a resolvable `CollectedSetId`, §0.6 invariant 3.)

### 0.4.5 Rust↔TS type-sharing strategy `[DECIDED — tauri-specta]`

The platform rule is **no `any`**; the Rust↔TS boundary must be typed with a
single source of truth. Options surveyed:

| Approach | Verdict |
|---|---|
| **Manual mirroring** | Rejected — guaranteed drift; violates the "no `any` by accident" intent. |
| **ts-rs** | Generates `.ts` from Rust types via derive, but treats types **individually** (a type and its dependency graph aren't exported together cleanly) and, critically, **does not model Tauri *commands or events*** — we'd still hand-write the `invoke`/Channel wrappers and could drift on argument names. |
| **specta** (alone) | The introspection layer ts-rs lacks (full type graph), but not Tauri-aware on its own. |
| **tauri-specta** (specta + Tauri integration) | **Recommended.** Purpose-built for Tauri v2: annotate commands with `#[specta::specta]`, collect via `collect_commands![]` / `collect_events![]`, and it emits a single `bindings.ts` exposing **typed `commands.*` wrappers, typed event/Channel helpers, and all referenced types** — exactly the C1–C13 + E-series surface above, with no `any` and no hand-written invoke glue. |
| **JSON-schema** | Heavier toolchain, no first-class Tauri command typing; rejected. |

**Decision `[DECIDED]`:** adopt **tauri-specta** (with specta). The spec already
leans on it everywhere (the §5.8 generated `commands.*`/`ConversionEvent` examples
assume it), so this is closed rather than left dangling. Generated output lands at a
single tracked path — **`src/lib/ipc/bindings.ts`** (the frontend's only door to the
backend; §5.1/§5.8 import from here and never call raw `invoke`). Generation runs as
part of the debug build / a dedicated `cargo` step; **§06 owns a CI drift check**
that fails if `bindings.ts` is stale vs the Rust source (regenerate +
`git diff --exit-code`).

**Held-in-reserve fallback (not a v1 open question):** if tauri-specta v2 proves
unstable against our pinned Tauri (§0.8), the documented fallback is **specta for the
types + a thin hand-written, drift-checked command map**. This is a contingency, not
an undecided choice — the default is tauri-specta and the §06 drift check guards it
either way. The *decision to codegen, not mirror* and the *tool* (tauri-specta) are
both `[DECIDED]`.

---

## 0.5 Conversion pipeline overview (navigational only)

> **This is a map. §01 is the canonical owner of the pipeline.** Nothing here is
> authoritative; it shows where the IPC commands (§0.4) hook into the §01 stages.

```
 drop / picker / launch-arg
        │  (C1 ingest_paths / C2a pick_for_intake)   §1.1 intake → §2.4 freeze
        ▼
 content detection (§1.2) ──► group by user-facing format (§1.3)
        │                                            mixed → MixedDrop refusal
        ▼
 collected-summary + confirm gate (§1.4)             (UI §5.2)
        │  (C3 get_targets)
        ▼
 target resolution + default (§1.5) + options (§1.6)
        │  (C4 plan_output)
        ▼
 output planning (§1.8) ──► re-run detection (§2.5) ──► resource pre-flight (§1.10)
        │  (C6 start_conversion + Channel<ConversionEvent>)
        ▼
 queue / job lifecycle (§1.9) ──► engine invocation (§1.7) ──► §0.9 pool
        │                            through §2.12 isolation, args §3.5
        ▼
 atomic write via guarantees-fs (§2.1/§2.3/§2.6/§2.7/§2.14)
        │  (E: ItemProgress / ItemFinished / BatchProgress)
        ▼
 end-of-batch summary (§1.12)  (C8 get_run_summary / E: RunFinished)  (UI §5.2)
```

---

## 0.6 Core domain model

The shared vocabulary. These are **Rust** types (the source of truth); the TS
mirror is generated (§0.4.5). `RunId`/`InstanceId` are **defined by §7.1** and
referenced here (this section does not own their identity policy). Fields are
illustrative-but-concrete; invariants are normative.

```rust
// ─── Identity (defined by §7.1; referenced here) ────────────────────────────
pub struct InstanceId(Uuid);   // one per app launch (§7.1)
pub struct RunId(Uuid);        // one per start_conversion (§0.4 C6 / §7.1)
pub struct CollectedSetId(Uuid);
pub struct ItemId(u32);        // stable within a run
pub type JobId = ItemId;       // §1.7/§1.8 say "JobId"; it IS the ItemId of the job's item
#[derive(Clone, Copy, Serialize, Deserialize, specta::Type)] // crosses IPC as a C1 arg (frontend-
                                          // generated, §0.4.1) AND C13 cancel_ingest arg → in
                                          // collect_types![] or the §0.4.5 drift check emits `any`
pub struct CollectingId(Uuid); // ingest-scoped cancellation handle, pre-RunId (§0.4 C13)
#[derive(Clone, Serialize, specta::Type)] // Channel<ScanProgress> payload MUST derive specta::Type
                                          // (in collect_types![]) or the C1 onScan payload is `any`.
                                          // PRECONDITION: typed Channel<T> serialisation requires the
                                          // `specta` feature on the tauri crate (enabled transitively
                                          // by tauri-specta's tauri dependency with features=["specta"]);
                                          // without it Channel<ScanProgress> is opaque in bindings.ts.
pub struct ScanProgress { pub scanned: u32 } // C1 onScan Channel payload (§0.4.2), throttled live count

// ─── Intake & detection ─────────────────────────────────────────────────────
pub enum IntakeOrigin { Drop, Picker, LaunchArg, SecondInstance } // §7.8

// ─── Wire DTOs for the C-commands + app:// hand-off (derive specta::Type; in
//     collect_types!). Defined here so every C1–C13 + app:// shape has one typed
//     home (no inline-comment-only types). camelCase on the wire. ─────────────
pub enum PickKind { Files, Folder }                 // C2a pick_for_intake `kind`
pub enum OpenKind { Folder, File, RevealInFolder }  // C9 open_path `kind` (§7.7)
pub struct IntakePayload {                           // app://intake hand-off (§7.8.1)
    pub paths: Vec<PathBuf>,
    pub origin: IntakeOrigin,                        // only LaunchArg | SecondInstance ever
                                                     //   appear in app://intake (§0.4.2 row):
                                                     //   Drop/Picker reach C1/C2a directly,
                                                     //   never via this event — a frontend
                                                     //   handler needs no Drop/Picker branch
}

pub struct DroppedItem {
    pub raw_path: PathBuf,        // as the OS handed it
    pub resolved_path: PathBuf,   // symlink/junction/alias-resolved (§2.3)
    pub size_bytes: u64,
    pub detected: DetectionOutcome, // §1.2 OWNS this type (the single canonical
                                  //   detection result); defined in §1.2, mirrored
                                  //   on the wire (§0.4.5). NOT a separate
                                  //   DetectedFormat — that earlier name is retired.
}

// `DetectionOutcome` + its `Confidence { High, Low }` are OWNED by §1.2 (the
// detection-algorithm owner) and referenced here, exactly like JobState/SkipReason
// patterns elsewhere. There is no `DetectedFormat`/`DetectionConfidence` pair — the
// earlier 3-valued confidence enum and the user_facing:Option collapse (which lost the
// Empty-vs-Unreadable distinction) are deleted in favour of §1.2's richer enum:
//   Recognized { format, confidence, dims: Option<(u32,u32)> } | UnsupportedType { detected } |
//   Uncertain { best_guess } | Empty | Unreadable { reason }.
//   (`dims` = header-derived raster width/height, §1.2 step 4 → the §1.10 cheap estimate input.)
// `SkippedItem` (below) projects an ineligible DetectionOutcome to a §2.8 reason.

/// The single grouping key (§1.3): individual user-facing format,
/// NOT the six SSOT categories, NOT codec subtypes. Jpg != Png, Mp4 != Mov.
pub enum UserFacingFormat { Jpg, Png, Webp, Gif, Bmp, Tiff, Heic, Avif, Ico, Svg,
    Mp3, Wav, Flac, Aac, M4a, Ogg, Opus, Wma, Aiff, Alac,
    Mp4, Mov, Mkv, Webm, Avi, Wmv, Flv, Mpeg, M4v, ThreeGp,
    Pdf, Docx, Doc, Odt, Rtf, Txt, Md, Html,
    Xlsx, Xls, Ods, Csv, Tsv,
    Pptx, Ppt, Odp }
// (The enumeration is the SSOT *What It Converts* set; 04-formats owns each one's
//  detection signature, targets, engine, options. This enum is just the key.)

// ─── Collected set (the frozen batch candidate) ─────────────────────────────
pub enum CollectedSet {
    Single {                         // exactly one user-facing format → a batch
        id: CollectedSetId,
        instance: InstanceId,
        format: UserFacingFormat,
        items: Vec<DroppedItem>,     // frozen, de-duplicated by resolved identity. Each carries
                                     //   its ItemId from the SINGLE id space over ALL dropped items
                                     //   (eligible + skipped); `items` is the ELIGIBLE filtered view
                                     //   — NOT re-indexed from 0 (§0.6 invariant 6).
                                     // raw_path SCOPE `[DECIDED]`: DroppedItem.raw_path IS on this
                                     //   wire type and reaches the WebView — but DISPLAY-ONLY (the
                                     //   §5.3 BatchSummary derives "e.g. holiday.jpg, cat.jpg"
                                     //   sample basenames from the first few items[].raw_path). It
                                     //   is NEVER re-submitted by the WebView as intake: the only
                                     //   intake funnels are C1 (paths the native drop/launch gave)
                                     //   and C2a (paths the Rust-opened picker gave), both
                                     //   Rust-side. The C2a "no raw FS path reaches the WebView"
                                     //   claim is scoped to the INTAKE-PICKER funnel (the WebView
                                     //   never SUPPLIES a path to re-ingest); a frozen set's
                                     //   raw_path travelling back for display does not let the
                                     //   WebView feed an arbitrary path into a conversion.
        count: usize,                // shown in the confirm gate (§1.4). INVARIANT `[DECIDED]`:
                                     //   count == items.len(), set ONCE at construction (the
                                     //   §1.1 freeze) and NEVER mutated independently. Kept as a
                                     //   separate field (not always derived) so a wire consumer
                                     //   reading the confirm tally never has to walk the full
                                     //   items Vec (a 10k-file batch); the §6 property-test
                                     //   asserts count == items.len() so the duplication can
                                     //   never silently drift.
        skipped: Vec<SkippedItem>,   // ineligibles dropped alongside the eligible set — the
                                     //   id-DISJOINT view over the same id space (their ItemIds
                                     //   never collide with eligible ones, §0.6 invariant 6);
                                     //   threaded through to the §1.4 confirm summary
                                     //   and the §1.12 RunResult ("N collected, M skipped")
        // ─ confirm-screen summary fields `[DECIDED]` — this IS the §1.4 CollectedSummary
        //   wire shape (the two are unified so the mandatory confirm gate has a real IPC
        //   path; §1.4 is the display/projection view of exactly these fields):
        total_bytes: u64,               // size hint / §1.10 pre-flight (§1.4)
        roots: Vec<PathBuf>,            // dropped root(s) → §2.7 subtree + open-folder
        encoding_hint: Option<String>,  // e.g. CSV detected "Windows-1252" (per 04)
        delimiter_hint: Option<String>, // e.g. CSV/TSV detected ";" (per 04)
        notes: Vec<CollectedNote>,      // §1.4-owned; PRODUCED by §1.2's bounded peek
    },
    Mixed { found: Vec<(UserFacingFormat, usize)> },  // → pre-flight refusal (§1.3)
    Unsupported { detected: String },                 // real but out-of-scope (§1.2)
    Uncertain { note: String },                       // can't tell (§1.2)
    Empty { skipped: Vec<SkippedItem> },              // nothing eligible — carries the per-item
                                                      //   skip reasons (§1.3 projection from
                                                      //   EmptyReport.outcomes) so the §5.2 state-10
                                                      //   copy can show "N files, none convertible
                                                      //   (M unreadable, K unsupported, …)" instead
                                                      //   of a reason-less empty (SSOT Fail-clearly).
                                                      //   The tally uses SkipReason (UnsupportedType
                                                      //   | Uncertain | Empty | Unreadable); hidden/
                                                      //   system files are walk-filtered and never
                                                      //   become SkippedItems (so an all-hidden drop
                                                      //   is Empty { skipped: vec![] }).
                                                      //   Empty-vec for the genuinely-zero-items case
                                                      //   (cancelled dialog / drained PendingIntake /
                                                      //   all files hidden-filtered).
}
// `CollectedSet::Single` carries the FULL confirm-summary field set, so it IS the wire
// shape C1/C2a return and the confirm gate (§1.4/§5.2) renders. `CollectedNote` is the
// §1.4-owned type (referenced here). The collected-set registry (§0.4.4) stores this
// payload + its roots keyed by `CollectedSetId` for C3/C4/C5/C6 to resolve.

// An item present in the drop but NOT eligible for the batch (unsupported / uncertain
// / empty / unreadable at freeze). Surfaced in the §1.4 confirm summary and the §1.12
// summary so a bad item is never silently dropped. Referenced by §1.3 Grouping::Single
// and §1.4 CollectedSummary.
pub struct SkippedItem {
    pub item: ItemId,                // stable within the collected set / run
    pub source: PathBuf,             // the dropped path, for the summary display
    pub reason: SkipReason,          // §0.6 SkipReason (UnsupportedType | Uncertain | Empty | Unreadable)
                                     //   — NOT ErrorKind. Every SkippedItem comes from a
                                     //   detection-INELIGIBLE outcome (§1.3), all of which have
                                     //   a SkipReason, so storing SkipReason makes the §1.12
                                     //   OutcomeMsg::Skipped projection a trivial copy (no lossy/
                                     //   undefined ErrorKind→SkipReason reverse map at the
                                     //   OutcomeMsg::Skipped boundary). §1.12 [DECIDED].
}
// The ONLY (one-way) conversion is the forward `SkipReason → ErrorKind`, used by the
// §1.12 projection helper when a Skipped item must also surface an ErrorKind-shaped
// display reason: `SkipReason::Uncertain` projects to `ErrorKind::Unrecognized`
// (ErrorKind has NO `Uncertain` variant — the can't-tell skip is surfaced as
// Unrecognized, §2.8.2); `UnsupportedType`/`Empty`/`Unreadable` map by identical name.
// This map lives on the PROJECTION HELPER (§1.12), not on the struct — and is never
// inverted (the non-injective `Uncertain→Unrecognized`, where `Unrecognized` also appears
// as a non-skip item error, would make any reverse map ambiguous; storing SkipReason
// avoids needing one).

// ─── Targets & options ──────────────────────────────────────────────────────
pub enum TargetId {                  // the offered-target identity (§1.5 TargetKind)
    Format(FormatId),                // a format target (e.g. Webp)
    Op(CrossCatOp),                  // a cross-category operation (ExtractAudio | ToGif)
}
pub type FormatId = UserFacingFormat; // a format target IS a user-facing format
pub enum CrossCatOp { ExtractAudio, ToGif } // closed set (cross-category.md)

pub enum Availability {              // from §3.4 patent disposition (resolved per platform)
    Available,
    Unavailable { reason: String },  // honest "unavailable here" (§3.4 / §5.2)
}

pub struct Target {                  // an offered output choice for a source
    pub id: TargetId,                // e.g. Format(Webp) | Op(ExtractAudio) | Op(ToGif)
    pub label: String,
    pub lossy: Option<LossyKind>,    // §2.9 catalog key (string lives in §2.9; the ONE canonical name)
    pub availability: Availability,  // from §3.4 (Available | Unavailable { reason })
    pub options: Vec<OptionDecl>,    // §1.6 generic model (OptionDecl); 04 owns concrete values
}

pub struct TargetOffer {
    pub set: CollectedSetId,
    pub targets: Vec<Target>,
    pub default_target: TargetId,    // exactly ONE pre-highlighted default (§1.5)
}

// The resolved option set for a batch. §1.6 owns the model; this is the ONE name
// for "the effective, fully-defaulted-plus-overrides values". §1.6's
// `EffectiveOptions` is the same type (a BTreeMap<OptionKey, OptionValue>); the
// wire/domain name is `OptionValues`.
pub struct OptionValues(BTreeMap<OptionKey, OptionValue>); // == §1.6 EffectiveOptions
// `LossyKind` (§2.9, owner), `OptionDecl`/`OptionKey`/`OptionValue`/`LabelKey`/
// `EnumChoice`/`Unit` (§1.6, owner — concrete defs there), `OutcomeMsg` (§2.8, owner
// — enum defined there). `AppInfo`/`EngineHealth` (§7.2, owner). `CollectedNote`
// (§1.4). `ReadFailure` (§1.2). `Platform`/`Direction`/`PatentDisposition`/
// `EngineCapability` (§3.2). All referenced here are defined by those owners; the
// wire mirror is generated (§0.4.5).

// ─── The batch & its jobs ───────────────────────────────────────────────────
pub struct Batch {
    pub id: CollectedSetId,
    pub source_format: UserFacingFormat,
    pub target: Target,              // INVARIANT: exactly one, whole-batch (below)
    pub options: OptionValues,       // INVARIANT: one effective set, whole-batch
    pub destination: DestinationChoice,
    pub jobs: Vec<ConversionJob>,
}

pub enum DestinationChoice {
    BesideSource,                    // default (§2.7); per-location divert applies
    ChosenRoot(PathBuf),             // re-creates relative subtree (§2.7)
}

pub struct ConversionJob {
    pub item: ItemId,
    pub source: DroppedItem,
    pub state: JobState,             // §1.9 owns the lifecycle transitions
    pub plan: Option<OutputPlan>,    // computed by §1.8 before write
}

// §1.9 owns the lifecycle TRANSITIONS; this is the canonical state type.
// `Failed` carries the §2.8 `ErrorKind` (the wire enum mirrored in §0.4.3) — NOT a
// full `IpcError` (the IpcError is assembled for the wire/summary from the kind +
// path + message; storing the kind keeps JobState cheap and serde-stable).
pub enum JobState {
    Pending,
    Running,
    Succeeded,
    Failed(ErrorKind),               // §2.8 kind; nothing written (§2.1)
    Skipped(SkipReason),             // detection-ineligible pre-flight (§1.2/§1.3)
    Cancelled,
}

pub enum SkipReason {                // why a pre-flight item never entered the queue (§1.3)
    UnsupportedType,                 // real but out-of-scope (§1.2)
    Uncertain,                       // can't tell (§1.2)
    Empty,                           // 0-byte / no decodable content
    Unreadable,                      // gone/locked/denied at freeze (§1.2)
}

// The coarse per-item progress stage, carried by ItemProgress (§0.4.2). §1.11 owns
// the per-engine semantics; this is the shared/wire enum name.
pub enum JobStage { Spawning, Decoding, Encoding, Writing }

// ─── Engine descriptor (the seam; §3.2 owns the registry/selection) ─────────
// The stable engine discriminant used in logging/SBOM/registry (§3.2 trait Engine
// `id()`, §3.7 SBOM rows). One variant per bundled engine; Ghostscript NOT shipped v1.
pub enum EngineId { FFmpeg, FFprobe, LibreOffice, Poppler, Pandoc, ImageMagick, ImageCore, NativeCsvTsv }
// NOTE — `ImageMagick` is a **bundled delegate inside the image-worker** (libvips
// `magicksave`/`magickload` for BMP+ICO, §3.5.5), NOT a registry-eligible engine: no
// (source,target) pair maps to `EngineId::ImageMagick` (BMP/ICO route through
// `ImageCore` = the image-worker), it has **no `EngineProgram`** and **no §3.2.3
// registry entry**, and there is **no `trait Engine` impl** for it. Its `EngineId`
// exists ONLY for SBOM/NOTICE attribution (§3.7) and the §7.2 EngineHealth
// presence-check. (Prevents a spurious `Engine` impl / registry row.)
// NOTE — `FFprobe` mirrors that same non-trait pattern: it is the video two-phase
// PROBE binary (`binaries/ffprobe`, §3.3.1), spawned as the §3.5.1 probe sub-invocation
// of the FFmpeg engine — NOT a registry-eligible engine in its own right (no
// (source,target) pair maps to it; the FFmpeg `trait Engine` impl owns the pair and its
// `plan()` returns the ffprobe `Invocation`). It has **no `EngineProgram`**, **no §3.2.3
// registry entry**, and **no `trait Engine` impl**; its `EngineId` exists so the
// sidecar-path resolver can locate `binaries/ffprobe` (distinct from `binaries/ffmpeg`,
// §3.3.1), for SBOM/NOTICE attribution (§3.7), and for the §7.2 EngineHealth
// presence-check. (Prevents a spurious `Engine` impl / registry row for the probe.)
// A capability descriptor, NOT a process and NOT the §3.2 `trait Engine` (the
// registry seam). The name is `EngineDescriptor` precisely to avoid colliding with
// that trait — §0.4/§0.6/§3.2/§3.5/§6.4/§07 refer to this domain type by this name.
pub struct EngineDescriptor {        // capability descriptor, NOT a process
    pub id: EngineId,                // FFmpeg | LibreOffice | Poppler | Pandoc | ImageCore | …
                                     //   (Ghostscript [DECIDED: NOT shipped v1] — §3.1/§3.6)
    pub serialised_only: bool,       // true for LibreOffice (§0.9)
    pub kind: EngineKind,            // Subprocess | InProcessNative (canonical name; mirrors §3.2 EngineProgram::InProcessNative — see §0.9 note)
}

// How an engine runs. Mirrors §3.2's `EngineProgram` at the domain level: every
// third-party engine (FFmpeg / LibreOffice / poppler / pandoc / ImageMagick and the
// libvips IMAGE-WORKER) is a Subprocess [DECIDED §0.6 note]; ONLY ConvertIA's own
// MIT native CSV/TSV engine (§3.5.6) is InProcessNative. There is NO in-process path
// for any third-party decoder of untrusted bytes (§2.12.4 absolute). The variant name
// `InProcessNative` is identical to §3.2 `EngineProgram::InProcessNative` (one canonical
// name for the same concept; the earlier `InCoreNative` spelling is retired).
pub enum EngineKind { Subprocess, InProcessNative }

// ─── Output plan & results ──────────────────────────────────────────────────
// OutputPlan is OWNED (computed) by §1.8; its canonical shape is copied here so
// the shared/wire type has one definition. It is DIRECTORY-BASED: the exact final
// name + no-clobber numbering is resolved LAZILY at write time on the resolved
// real file via §2.1's exclusive create — NEVER a pre-baked `final_path` string
// (a pre-numbered path would reintroduce the TOCTOU race §2.1.2 eliminates).
pub struct OutputPlan {              // computed by §1.8, consumed by §2.1/§2.14; §2.7 rules
    pub job: JobId,
    pub final_dir: PathBuf,          // beside-source OR diverted (§2.7)
    pub diverted: Option<DivertReason>, // unwritable / ephemeral (§2.7); None = beside-source
    pub base_name: OsString,         // SOURCE base name kept (§2.2)
    pub extension: OsString,         // from the chosen TARGET (§2.2)
    pub publish_temp_dir: PathBuf,   // EQUALS final_dir in v1 (§2.14.1): the kind-1 `*.part` is a
                                     //   sibling DOTFILE here, NOT a per-run scratch SUBDIR. Same
                                     //   volume as final_dir. (Kind-2 engine-working scratch root,
                                     //   §2.14.2, may be on another volume and is NOT in OutputPlan.)
    // NOTE: cross-volume is NOT pre-planned in v1 `[DECIDED]` — meaning ONLY that there
    // is no stored `crosses_volume` field on OutputPlan; the plan never PREDICTS a
    // cross-volume publish. The PUBLISH path is reactive: `fs_guard::atomic_publish`
    // tries the direct intra-volume publish and falls back to copy-into-dest-volume ONLY
    // on EXDEV / cross-device failure (§2.14.3). (This near-never fires on the common
    // path: §2.1.1 step 1 / §2.14.1 place the publish temp as a SIBLING of `final` on
    // `final`'s own volume by construction, so the publish rename is intra-volume.) The
    // genuinely cross-volume case is the ENGINE-SCRATCH placement, which IS a pre-engine
    // decision (where the engine is told to write when a same-volume sibling temp cannot
    // be created) — but that placement is owned by §2.14.3 at run time, not stored as a
    // plan field. So "not pre-planned" = no plan field, NOT "no pre-engine decision".
    // NOTE: no `final_path`/`temp_path` — the numbered final name is produced at
    // write time (§2.1 exclusive create_new loop), never stored in the plan.
}

pub enum DivertReason { Unwritable, Ephemeral, NoAtomicPublish }  // §2.7.2 classification
// NoAtomicPublish (Unix-only): destination filesystem accepts a create but offers NO
// create-only/atomic no-clobber publish primitive — neither RENAME_NOREPLACE-class
// no-replace rename NOR hardlinks (FAT/exFAT-class, the canonical portable-USB case,
// §2.14.2). Diverted to a hardlink-capable system-disk target (§2.7.3) so the full §2.1
// publish chain holds. Windows is unaffected (MoveFileExW create-only works on FAT/exFAT).

// ─── Command return DTOs (the wire shapes C4/C5/C6 return — §0.4.1) ──────────
pub struct OutputPlanPreview {       // C4 plan_output → drives the "will save to…" line
    pub set: CollectedSetId,
    pub final_dir_preview: PathBuf,  // resolved destination shown before convert (§1.8/§2.7)
    pub diverted: Option<DivertReason>, // any per-location divert previewed (§2.7)
    pub rerun: Option<RerunPrompt>,  // Some(..) if §2.5 detected an equivalent prior run
    pub preflight: PreflightVerdict, // §1.10 size/space estimate + any up-front "too big" fail
}

pub struct RerunPrompt {             // the one batch-level §2.5 prompt's data
    pub equivalent_count: usize,     // how many items in the batch are flagged equivalent (§2.5)
}

pub enum RerunDecision { Skip, FreshCopy } // C6 input: skip (safe default) | make fresh copies (§2.5)

pub struct PreflightVerdict {        // §1.10 (owner) summary surfaced before convert
    pub est_total_output_bytes: u64,
    pub est_total_scratch_bytes: u64,
    pub up_front_fail: Option<ErrorKind>, // Some(TooBig|OutOfDisk) ONLY for the WHOLE-BATCH
                                     //   doomed case (the §5.2 disable-Convert-wholesale
                                     //   flag). OutOfDisk fires when ANY ONE PHYSICAL VOLUME's
                                     //   grouped footprint cannot fit its free space — the check
                                     //   is PER-PHYSICAL-VOLUME, split by category: est_output +
                                     //   publish temp → each item's final_dir volume; est_scratch
                                     //   (kind-2) → the system/scratch volume (§2.14.2), which is
                                     //   NOT necessarily the destination. (§2.7 beside-source/
                                     //   divert spread a batch across 2+ destination volumes;
                                     //   §1.10 / §2.14.4.) TooBig =
                                     //   the absolute per-item/aggregate output ceiling. A
                                     //   PER-ITEM too-big / out-of-disk is NOT carried here: it
                                     //   is enforced at WRITE TIME (mid-run) as that item's
                                     //   Failed(TooBig|OutOfDisk) while the batch continues
                                     //   (§1.10 / §1.11 fast-fail surfacing). So "preferably up
                                     //   front" = the per-volume whole-batch verdict here +
                                     //   per-item enforcement at the §2.1 write.
    // v1 SURFACING SCOPE `[DECIDED]`: the check is computed PER-PHYSICAL-VOLUME (above), but
    //   v1 surfaces only the BOOLEAN verdict (up_front_fail Some/None) + the AGGREGATE totals
    //   (est_total_output_bytes / est_total_scratch_bytes) to §5.2 — it does NOT carry a
    //   per-volume breakdown, so the UI cannot NAME the short volume in the doomed-1GB-USB
    //   case (it says "won't fit", not "the USB is the one that's short"). A per-volume
    //   breakdown (Vec<{ volume, free, needed }>) so §5.2 can name the short volume is
    //   [DEFER: post-v1] — v1's boolean+aggregate is the SSOT "fails fast up front" floor.
}

pub struct DestinationResolved {     // C5 set_destination → revalidated destination
    pub destination: DestinationChoice,
    pub diverted: Option<DivertReason>, // recomputed per-location divert (§2.7)
    pub preflight: PreflightVerdict, // RE-EVALUATED for the new destination volume
                                     //   (§2.14.4 free-space targets the destination;
                                     //   §1.8 destination-change re-validation) so the
                                     //   UI's held C4 verdict never goes stale
    pub rerun: Option<RerunPrompt>,  // CARRIED THROUGH UNCHANGED from the C4 verdict.
                                     //   In v1 the §2.5 EquivKey has NO destination
                                     //   component, so re-run is destination-INDEPENDENT
                                     //   (§2.5.1). C5 re-evaluates ONLY `preflight` (the
                                     //   destination-volume free-space check); it never
                                     //   recomputes `rerun`.
}

pub struct RunResult {               // canonical shape; §1.12 computes & references by name
    pub collected_set_id: CollectedSetId, // Batch.id is a CollectedSetId (§1.12)
    pub run_id: RunId,               // §7.1
    pub items: Vec<ItemResult>,      // per-item outcome + output→source mapping (§1.12).
                                     //   INCLUDES the freeze-time pre-flight SkippedItems
                                     //   (CollectedSet.skipped) projected as ItemResult
                                     //   { state: Skipped(reason), output: None,
                                     //     reason: Some(OutcomeMsg::Skipped{ reason, .. }) } —
                                     //   skip rides the skip-shaped OutcomeMsg variant (§2.8),
                                     //   NOT Failure, so skip != fail at the type level —
                                     //   §1.12 `[DECIDED]`; Totals.skipped counts them.
    pub totals: Totals,              // succeeded / failed / cancelled / skipped (§1.12)
    pub cleanup_incomplete: Vec<CleanupResidue>, // §2.6 cleanup-incomplete warnings
    pub common_root: PathBuf,        // "open folder" target for the BESIDE-SOURCE outputs
                                     //   (the dropped-selection common ancestor, §2.7 / §7.7)
    pub divert_root: Option<PathBuf>,// Some(Downloads/Documents/chosen) when ANY item was
                                     //   diverted (§2.7.3) — a SINGLE PathBuf cannot carry both
                                     //   roots, so the divert root is its own field. None when no
                                     //   item diverted. Both roots are §7.7.3 open-folder targets;
                                     //   per-item diverted outputs are also reachable via
                                     //   ItemResult.output (C9 open_path, kind=RevealInFolder,
                                     //   via OpenerExt::reveal_item_in_dir). (§1.12 / §7.7.3)
}

pub struct ItemResult {              // §1.12
    pub source: PathBuf,             // for output→source mapping
    pub state: JobState,
    pub output: Option<PathBuf>,     // Some(..) only when Succeeded
    pub reason: Option<OutcomeMsg>,  // §2.8 failure string OR §2.9 lossy note (link)
}

pub struct Totals { pub succeeded: u32, pub failed: u32, pub cancelled: u32, pub skipped: u32 }
// `all_failed` is DERIVED (failed == total && total > 0), not a stored field.

pub struct CleanupResidue {          // §2.6.4 residue-may-remain case
    pub item: ItemId,
    pub residue_path: PathBuf,
}

// The terminal per-item outcome carried by ItemFinished (§0.4.2).
pub enum ItemOutcome {
    Succeeded { output_path: PathBuf },
    Failed { error: IpcError },      // §0.4.3
    Skipped { reason: SkipReason },
    Cancelled,
}
```

**Invariants (normative):**

1. **One `Target` per `Batch` (v1).** `Batch.target` is a single value applied to
   every `ConversionJob` in the batch. There is no per-item target — enforced by
   the absence of any per-item-target IPC command (§0.4) and by `start_conversion`
   taking one `target`. (SSOT *How It Feels* 4: "one chosen target applies to the
   whole same-source batch".)
2. **One effective `OptionValues` per `Batch`.** Same rationale; also what §2.5
   keys "same effective settings" on.
3. **A `Batch` exists only from a `CollectedSet::Single`.** `Mixed`/`Unsupported`/
   `Uncertain`/`Empty` never produce a batch — they are pre-flight terminal states
   (§1.3 refusal / §1.2 decline). No subset conversion.
4. **The `items` set is frozen and resolved-identity-deduplicated** at ingest
   (§2.4/§2.3); nothing is added after the freeze, including outputs landing in a
   source folder.
5. **`OutputPlan.publish_temp_dir` (where the kind-1 `*.part` lives — EQUALS `final_dir`
   in v1, the `*.part` being a sibling dotfile, not a subdir, §2.14.1) and `final_dir` are
   on the same filesystem** (§2.14) so the §2.1 publish is a true intra-volume atomic
   rename; the
   exact numbered final name is resolved at write time, never stored. When the only
   obtainable scratch spans volumes, the **PUBLISH** detects this **reactively on EXDEV /
   cross-device failure** (`fs_guard::atomic_publish`, not via a pre-planned flag) and
   runs the §2.14.3 copy→fsync→exclusive-rename-within-destination fallback. "Not
   pre-planned" means **no stored `crosses_volume` field** — it does NOT mean there is no
   pre-engine decision: the choice of **where the engine writes** when a same-volume
   sibling temp cannot be created IS made before the engine runs (the engine is pointed at
   an other-volume scratch), owned by §2.14.3 at run time, not as an `OutputPlan` field.
6. **`ItemId` is stable within a `RunId`** so progress/finished events and the
   summary all address the same item. **`ItemId` is assigned at the §1.1 freeze**
   (collected-set) as the stable index of each item in **the de-duplicated frozen `Vec`
   of ALL dropped items — eligible AND skipped alike** (§2.4), assigned **once** over
   that single id space. `CollectedSet::Single.items` (eligible `DroppedItem`s) and
   `.skipped` (ineligible `SkippedItem`s) are **id-disjoint filtered VIEWS over that one
   id space** — they are **never re-indexed from 0**, so a `SkippedItem.item` can never
   collide with an eligible item's id, and §1.12 can project the skipped items into
   `RunResult.items` without an id clash. The id is identical through `Batch`/`Run` and
   every per-item event (`SkippedItem` pre-`RunId`, `ItemProgress`/`ItemFinished` in-run).

The **detection algorithm** (§1.2), **lifecycle transitions** (§1.9), **engine
selection** (§3.2), **per-format options/defaults** (04-formats), **output-naming
mechanics** (§2.2) and **identity policy** (§7.1) are owned by those sections;
this model only fixes the *shapes and invariants* the whole system shares.

---

## 0.7 Project layout & logical module decomposition

### Logical modules (the architecture — owned here)

Dependencies point **downward only**; nothing below depends on anything above it
(so the directory tree does not silently *become* the architecture). The
**guarantees-fs** layer and the **engine-registry seam** are the two reuse hubs.

```
            ┌─────────────────────────────────────────────┐
   tier 0   │  ipc  (Tauri command/event handlers, §0.4)  │  ← WebView talks only here
            └───────────────┬─────────────────────────────┘
                            │ depends on
            ┌───────────────▼─────────────────────────────┐
   tier 1   │  orchestrator  (queue, job lifecycle §1.9,   │
            │   run registry + cancellation tokens §0.4.4, │
            │   progress fan-out to the Channel)           │
            └───────┬───────────────┬───────────────┬──────┘
                    │               │               │
        ┌───────────▼───┐  ┌────────▼───────┐  ┌────▼──────────────────┐
 tier 2 │  detection    │  │ engine-registry│  │  guarantees-fs        │
        │  (§1.2)       │  │  seam (§3.2)   │  │  (no-clobber/atomic/   │
        │               │  │  + invocation  │  │  resolved-id/frozen/   │
        │               │  │  (§1.7) + args  │  │  cleanup/destination/  │
        │               │  │  (§3.5) +       │  │  temp §2.1/2.3/2.4/    │
        │               │  │  isolation seam │  │  2.6/2.7/2.14)         │
        │               │  │  (calls §2.12)  │  │                        │
        └───────┬───────┘  └────────┬───────┘  └───────────┬───────────┘
                │                   │                       │
        ┌───────▼───────────────────▼───────────────────────▼───────────┐
 tier 3 │  domain  (§0.6 types) + errors (§2.8 taxonomy)                │
        │  + platform util (paths, volume detection §2.14, OS shims)    │
        └──────────────────────────────────────────────────────────────┘
            ┌─────────────────────────────────────────────┐
   tier 3  │  subprocess pool  (§0.9) — used by engine-     │  (sibling of guarantees-fs;
            │  registry invocation; owns concurrency degree │   depended on by tier 2 engine seam)
            └─────────────────────────────────────────────┘
```

**Module responsibilities & who owns the behaviour:**

- **`ipc`** — the §0.4 command/event handlers; the *only* module the WebView
  reaches. Thin: validate, delegate to `orchestrator`, map `Result` → `IpcError`.
- **`orchestrator`** — the §01 pipeline conductor: builds the queue, drives
  `JobState`, holds the run registry + cancellation tokens (§0.4.4), and fans
  progress out to the Channel. Owns nothing the guarantees/engines own; it
  *sequences* them.
- **`detection`** — §1.2 content sniffing. First code to touch untrusted bytes;
  §1.2 owns whether header sniffing sits inside/outside the §2.12 boundary.
- **`engine-registry seam`** — the §3.2 `Engine` trait + registry + selection, the
  §1.7 generic invocation lifecycle, and §3.5 per-engine arg construction; every
  spawn routes through the §2.12 isolation wrapper and the §0.9 pool. This is the
  reusable engine home — adding a format pair is (mostly) a registry entry.
- **`guarantees-fs`** — the **reusable home of the no-harm machinery**:
  no-clobber/atomic write (§2.1), resolved-identity & link safety (§2.3), frozen
  set (§2.4), cleanup/temp ownership (§2.6), destination/divert (§2.7), cross-
  volume strategy (§2.14). Every output flows through here; **engines never write
  the final file** — they write to a temp the guarantees-fs layer owns, which then
  performs the atomic publish.
- **`domain`** — the §0.6 types + §2.8 error taxonomy; depended on by everyone,
  depends on nothing.
- **`subprocess pool`** — §0.9; the concurrency-degree owner and the per-engine
  parallelism rules (LibreOffice serialised).

### Physical tree (mapping the logical modules onto disk)

```
convertia/
├─ src-tauri/                      # the Rust core + Tauri host (the binary)
│  ├─ Cargo.toml                   # workspace root or member; pinned versions §0.8
│  ├─ tauri.conf.json              # bundle, CSP, externalBin, minimum-OS (§0.10, §0.3.1, §3.3)
│  ├─ build.rs                     # tauri-build; (optionally) tauri-specta gen hook
│  ├─ capabilities/
│  │  └─ main.json                 # the §0.10 capability allowlist (core, log, store — NO dialog, NO opener, NO shell-execute, NO fs; dialog/opener are Rust-side-only, not WebView grants, §3.3.3)
│  ├─ binaries/                    # bundled engine sidecars per platform (§3.3), externalBin targets
│  │  ├─ ffmpeg-x86_64-pc-windows-msvc.exe  (etc. — target-triple-suffixed)
│  │  ├─ ffprobe…  soffice…  pdftotext…  pandoc…  (per-platform; §3.1/§3.3)
│  │  ├─ convertia-imgworker-<triple>[.exe]  # the libvips IMAGE-WORKER process (§0.9/§3.5.5)
│  │  │                                      #   — a packaged externalBin (NOT linked into the core),
│  │  │                                      #   resolved Rust-side via current_exe().parent() (§3.3.3);
│  │  │                                      #   links libvips/libheif/libde265/librsvg/ImageMagick (§3.6.1)
│  ├─ resources/                   # bundled non-exe engine assets (LibreOffice profile seed, fonts §documents.md, image codec libs)
│  └─ src/
│     ├─ main.rs                   # Tauri builder, invoke_handler (C1–C13), collect_commands!/collect_events! (§0.4.5)
│     ├─ ipc/                      # tier 0 — §0.4 handlers, one file per command group
│     ├─ orchestrator/             # tier 1 — queue, lifecycle (§1.9), run registry, cancellation (§0.4.4)
│     ├─ detection/                # tier 2 — §1.2
│     ├─ engines/                  # tier 2 — registry/seam (§3.2), invocation (§1.7), args (§3.5), per-engine modules
│     │  ├─ registry.rs            #   Engine trait + selection (the §3.2 seam — candidate own crate)
│     │  ├─ invoke.rs              #   §1.7 generic lifecycle (spawn/progress/cancel/timeout/error-map)
│     │  ├─ ffmpeg.rs  libreoffice.rs  pandoc.rs  poppler.rs  image.rs  csv_native.rs
│     ├─ fs_guard/                 # tier 2 — the reusable guarantees-fs layer; module path `crate::fs_guard` (§2.0); §2.1/2.3/2.14 atomic write/no-clobber/resolved-id/path-limit/cross-volume
│     ├─ run/                      # tier 2 — `crate::run` (§2.0): per-run/instance scratch ownership + cleanup (§2.4/§2.6), keyed on RunId/InstanceId (§7.1)
│     ├─ outcome/                  # tier 2 — `crate::outcome` (§2.0): the §2.8 error taxonomy + message catalog AND the §2.9 lossy catalog ↔ IpcError mirror (§0.4.3); the single source of every conversion-outcome string (was `error.rs` — RENAMED to match `crate::outcome` in §2.0; there is no `crate::error`)
│     ├─ isolation/                # tier 2 — `crate::isolation` (§2.0): the §2.12 decoder-isolation wrapper every engine spawn routes through (§1.7 calls it; §3.5 builds args inside it)
│     ├─ pool/                     # tier 3 — subprocess pool, concurrency degree (§0.9)
│     ├─ domain/                   # tier 3 — §0.6 types, derive specta::Type
│     └─ platform/                 # tier 3 — path/volume/OS shims (§2.14, §7.7 reveal-in-folder)
│
├─ src/                            # the React 19 / TS / Tailwind / Vite UI (§05)
│  ├─ lib/ipc/bindings.ts          # GENERATED by tauri-specta (§0.4.5) — the only IPC door
│  ├─ components/  hooks/  state/  styles/   # §5.x owns these
│  └─ main.tsx
│
├─ index.html  vite.config.ts  package.json  tsconfig.json   # frontend build
├─ tests/                          # Rust integration + corpus harness (§6.4); guarantees property tests
└─ scripts/                        # build/bundle/SBOM/checksum (§06)
```

**Engine-registry-as-crate `[OPEN → recommend: module first, extract later]`:**
the §3.2 seam *could* be its own crate (`convertia-engines`) to enforce the
dependency direction at the compiler level. Recommendation: **start as a module**
(`src-tauri/src/engines/`) and extract to a workspace crate only if a second
consumer (e.g. a headless test harness) appears. Flagged for §3.2/§0.7 sign-off.

> **Note — image codecs run in a separate image-worker process `[DECIDED]`.** Unlike
> FFmpeg/LibreOffice/pandoc/poppler (clearly separate binaries), the image core
> (libvips + libheif/libde265 + the librsvg SVG load module + cgif, per images.md)
> *could* be linked as a Rust crate **or** run out-of-process. The **isolation
> requirement (§2.12) for untrusted image bytes** (the T1 headline threat — a
> libvips/libheif/librsvg memory-corruption exploit must not run inside the ConvertIA core address
> space) settles it: **v1 runs image decode/encode in a separate short-lived
> image-worker process**, so a hostile-image exploit is contained by the same OS
> process boundary as every other engine and §2.12.4's "all decoders are
> subprocesses" stays true. (§3.6 licensing is unaffected — libvips is LGPL either
> way; this is a security/robustness call, now resolved.) The image-worker still
> *links* libvips/LGPL libs internally, which is aggregation, not a link into the MIT
> core (§3.6.1). The `EngineKind` field on the §0.6 `EngineDescriptor` records the
> image core as `Subprocess` (the worker process); only the native CSV/TSV engine
> (§3.5.6) is `InProcessNative`.

---

## 0.8 Tech stack & pinned versions

`[DECIDED]` framework & language; `[DEFER: build]` exact patch pins (locked at first build,
recorded in lockfiles + the SBOM, §6.3). Versioning policy: **pin everything**
(Cargo.lock + pnpm-lock committed); bumps are deliberate and re-validated against
the corpus (§6.4) — engine bumps are best-effort posture (§3.8), not a gate.

| Layer | Choice | Pin policy |
|---|---|---|
| Rust toolchain | stable (recommend a recent stable, e.g. `1.8x` class as of build) via `rust-toolchain.toml` | pinned channel |
| Tauri | **v2** (`tauri` 2.x, `tauri-build`, `@tauri-apps/api` 2.x, **`@tauri-apps/cli` 2.x** — the devDependency that RUNS `tauri dev`/`tauri build`, matched to the `tauri` 2.x pin) | exact, lockfile |
| Async runtime | **tokio** (multi-thread) — Tauri's async commands run on it; subprocess IO + Channel feed off it | exact |
| IPC type-gen | **tauri-specta** + **specta** (§0.4.5, `[DECIDED]`) | exact |
| Cancellation | **tokio-util** (`CancellationToken`) | exact |
| Error plumbing | **thiserror** (core error enums) → mapped to `IpcError` (§0.4.3); `serde` for wire | exact |
| Detection | content-sniffing crate(s) — `infer` and/or hand-rolled magic tables; §1.2 owns the strategy | exact |
| FS guarantees | `tempfile` (owned scratch), `same-file`/`dunce` (resolved-identity, Windows path canonicalisation), `fs2`/platform calls (free-space), atomic rename via std + §2.14 cross-volume fallback | exact |
| Frontend | **React 19**, **TypeScript** (strict, no `any`), **Vite** (per platform CLAUDE.md, current major), **Tailwind CSS** | exact, lockfile |
| Frontend state | lightweight store (recommend **Zustand**) + the generated `bindings.ts`; §5.1 owns the final choice | §5.1 |
| Package mgr | **pnpm** (`pnpm@10.13.1` class per platform standard) | pinned |
| Test | **Vitest** (frontend) + **`vitest-axe@0.1.0`** (real npm pkg, Vitest-native `jest-axe` fork; deps `axe-core ^4.4`; Lane-A ARIA/role/focus, §6.4.6a — bump to the `1.0.0-pre` line if it stabilises pre-Phase-3), **cargo test** + corpus harness (§6.4), property tests for guarantees; **E2E = WebdriverIO v9** (W3C-only, `tauri-driver`-aligned) + **`@axe-core/webdriverio`** (Lane-B live-WebView contrast gate, §6.4.6/§6.4.6a) | exact, lockfile |
| Engines (bundled) | FFmpeg (GPL-2.0+ build — enables x264, §3.6.1), LibreOffice, poppler, pandoc, ImageMagick (required, permissive), libvips+libheif/libde265+x265-plugin/libaom/dav1d+librsvg+cgif — **all §3.1/§3.3 owned**; versions pinned + in the SBOM (§6.3). Ghostscript **[DECIDED: dropped v1]** (§3.1). | §3.8 best-effort |

**Additional crates / plugins other sections depend on (pinned, in lockfile + SBOM):**

| Crate / plugin | Used by | Why |
|---|---|---|
| **process-wrap** | §1.7 | cross-platform process-group / Job-Object spawn+group-kill (engine tree teardown) |
| **walkdir** | §1.1 | ergonomic recursive folder enumeration (Rust-side intake) |
| **chardetng** | §1.2 | text-encoding detection for the magic-less formats |
| **flate2** (`rust_backend`/miniz_oxide feature ONLY — pure safe Rust, NO zlib/zlib-ng C backend) | §1.2 | bounded in-core `.svgz` (1F-8B) inflate for content detection (≤64 KiB + ≤100× ratio cap); pure-Rust so the §2.12.4 "no third-party C/C++ decoder in-core" absolute holds |
| **tauri-plugin-single-instance** | §7.1 | single-instance policy + launch-arg hand-off |
| **tauri-plugin-dialog** | §0.4.1 C2a/C2b, §1.1, §5.4 | native file/folder picker via `DialogExt` (`app.dialog().file().pick_file(..)` / `.pick_folder(..)`), called **Rust-side** from the C2a/C2b handlers — **no `dialog:allow-open` WebView grant** (the `dialog:*` capability is only for the JS guest bindings, which ConvertIA does not use). Registered via `tauri_plugin_dialog::init()` in the §7.x Builder. |
| **tauri-plugin-store** | §7.4 | the single `settings.json` prefs blob (theme + lastDestinationMode + verboseLog) |
| **tauri-plugin-log** | §7.5 | local-only rotating diagnostic log + JS bridge |
| **tauri-plugin-opener** | §7.7 | open-folder / open-file / open-url shell-out (the only OS shell-out) — called **Rust-side via `OpenerExt`** from the C9/C10 handlers (no WebView `opener:*` grant, §0.10/§7.7.1) |

Concrete crate **versions are deliberately not hard-coded in this prose** (they go
stale); the lockfiles + SBOM are the source of truth (§6.3). This table fixes the
*choices*, not the digits.

---

## 0.9 Concurrency, threading & engine-subprocess pool — **owner of the concurrency degree**

**Async runtime.** tokio multi-threaded. Tauri commands are `async` and return
quickly (C6 returns a `RunId` immediately; the run proceeds in the background and
streams over the Channel) so the WebView never blocks (SSOT *stays responsive*).

**The pool & the single concurrency-degree number.** A bounded **engine-subprocess
pool** governs how many engine processes run at once. **This number lives here;
§1.10 references it for budgets, §1.11 for batch progress.**

`[DECIDED] default concurrency policy:`

- **Global degree = `clamp(physical_cores − 1, 1, 4)`**, default-capped low because
  the heaviest engines are CPU-bound (video re-encode) and we must keep the app
  responsive and the machine usable. A sensible everyday default is **2–4**; the
  cap of 4 prevents a 16-core machine from spawning 16 FFmpeg re-encodes and
  thrashing.
- **Per-engine parallelism overrides the global degree where correctness or
  resource pressure demands:**

| Engine | Parallelism | Rationale |
|---|---|---|
| **LibreOffice** (`soffice --headless`) | **serialised — exactly 1 at a time** `[DECIDED]` | LibreOffice headless is **NOT safely parallel under one user profile**: concurrent `soffice` instances sharing a profile **lock/corrupt** it — a *correctness* issue, not just contention. The pool runs a dedicated **single-slot LibreOffice lane**; all office/PDF-export jobs (documents/spreadsheets/presentations) serialise through it. Mitigation detail (per-run isolated `-env:UserInstallation` profiles) is co-owned with §3.5; even with isolated profiles the safe v1 stance is **one office conversion at a time**. |
| **FFmpeg** (video re-encode) | **low — 1–2** | CPU-bound; already the slowest op (video.md). Counts against the global degree. |
| **FFmpeg** (audio / extract-audio / remux) | up to global degree | light/IO-bound; may run more in parallel. |
| **Image core** (vips/heif/avif/svg) | up to global degree | per-item, bounded-memory (vips streaming); fast. Runs as a **separate image-worker process** `[DECIDED]` (§0.7/§2.12), one short-lived worker per item, so a hostile-image decoder exploit is process-isolated like every other engine. |
| **poppler / pandoc** | up to global degree | light, short-lived. |
| **native CSV/TSV** (in-Rust, no subprocess) | up to global degree (worker threads) | trivial cost. |

- **Effective parallelism = `min(global_degree, per_engine_cap)`.** The per-engine
  caps above **override** the global degree downward, never upward: e.g. video
  re-encode runs at `min(global_degree, 2)`, LibreOffice at exactly 1 regardless of
  the global degree. A batch mixing engines respects each engine's own cap within
  the shared global bound.
- **`EngineDescriptor.serialised_only` enforcement mechanism `[DECIDED]`.** For an
  engine whose descriptor has `serialised_only = true` (LibreOffice), the pool holds a
  **dedicated single-permit semaphore** (one per serialised engine). A job for that
  engine must **acquire BOTH** the global degree semaphore **and** that engine's
  single-permit semaphore **before spawn**, and **releases both on subprocess exit**
  (success/fail/kill). This is the concrete code that *reads* `serialised_only`: the
  pool, at registry-build time, allocates a `Semaphore(MAX_LO_CONCURRENCY)` for each
  engine flagged serialised; non-serialised engines acquire only the global degree permit.
  **`MAX_LO_CONCURRENCY = 1` is a §0.9-owned `pub const` `[DECIDED]`** (the single source
  of the LibreOffice serialisation degree); the §6.7.2 test harness **imports this same
  constant** rather than hard-coding `1`, so the test env can never drift from prod.
  **How the pool gets `serialised_only` from a running job's `EngineId` `[DECIDED]`:**
  the §3.2.3 registry maps `(SourceFmt,TargetFmt) → EngineId`, and the §3.2 `trait
  Engine` exposes **`fn descriptor() -> EngineDescriptor`**; the pool reads
  `registry.engine(engine_id).descriptor().serialised_only` before dispatch (or, at
  registry-build time, pre-computes a `HashMap<EngineId, bool>` of serialised flags
  from each registered engine's `descriptor()`, read on every dispatch). This is the
  named `EngineId → serialised_only` path — there is no descriptor-less lookup gap.
- **FFmpeg internal threading (avoid oversubscription).** FFmpeg's own
  `libx264`/`libvpx` use multiple internal threads per process by default, so even
  the **1–2** video-re-encode cap can saturate the CPU. v1 does **not** additionally
  cap FFmpeg's `-threads` (its internal threading is what makes a single re-encode
  fast); the **1–2** cap is set *because* one or two FFmpeg processes already use
  most cores. Net: video re-encode is effectively serial-ish on typical machines,
  by design — not a bug. (If profiling later shows oversubscription on
  many-core machines, capping `-threads` per process is the lever — recorded, not v1.)
- **libvips internal threading (image-worker oversubscription) `[DEFER: profile]`.**
  Analogous to the FFmpeg case: **libvips spawns its own internal thread pool per
  image-worker process** (its `vips_concurrency` default ≈ core count). If the §0.9 image-core
  pool runs **N** image workers concurrently, the effective thread count is
  **N × libvips-threads**, which on a many-core machine can far exceed physical cores. v1 does
  not cap this by default (a single image op finishing fast is usually the win), but if
  profiling against the §6 corpus shows N-worker oversubscription, the levers are
  **`VIPS_CONCURRENCY=1` per worker** (in the worker's whitelisted env, distinct from the
  stripped `LD_*`/`DYLD_*` vars, §2.12.3) **or** lowering the image-worker global-degree cap.
  Recorded as the lever, not a v1 commitment. Owner: §0.9 (co-ref §3.5.5).
- **Timeout / hang policy parameters.** The pool carries the *parameters* (per-
  engine wall-clock timeout, hang detection via no-progress watchdog); the
  **mechanism** (how a timed-out/hung engine is killed and mapped to §2.8) is
  **owned by §1.7** and referenced here. The concrete values are **named `pub const`s in
  this §0.9 pool module** (co-located with `MAX_LO_CONCURRENCY`, and **imported by the §6.7.2
  test harness** so test and prod can never drift): a **per-engine wall-clock timeout**
  (generous for video — a long film legitimately takes minutes — tight for the light
  engines), the **watchdog poll interval**, and the **no-progress threshold** (time without
  stdout/stderr/output-size progress before a hang is declared). v1 ships **baseline values
  calibrated against the §6 corpus**, and a committed **timeout-sentinel corpus case** (a
  deterministic input / a `#[cfg(test)]` sidecar that reliably exceeds the budget or stalls
  without progress) exercises the §1.7 reap so the parameters are test-covered, not prose.
- **Panic isolation.** A worker thread driving a job wraps its body so a Rust-side
  panic surfaces as a clean per-item `Failed` (§2.13 `catch_unwind`/isolate-and-
  report), never poisoning the pool. (Mechanism owned by §2.13.)

**Binding to identity & temp.** Each running job is `(InstanceId, RunId, ItemId)`
(§7.1) and writes only into its **per-run owned scratch** (§2.6/§2.14), so parallel
jobs — and a second app instance, if §7.1 allows one — never collide on temp files
and cleanup never removes another job's in-progress file.

---

## 0.10 Tauri security boundary — capabilities/permissions allowlist + CSP `[DECIDED]`

This is the **WebView half** of security (the WebView is untrusted; the
capabilities system is the contract for what it may ask the core to do). The
**subprocess/decoder half** is §2.12. Together they form the §0.11 map.

**Capability allowlist (`src-tauri/capabilities/main.json`)** — *deliberately
minimal, deny-by-default.* The WebView is granted **only** what the §0.4 commands
need:

```jsonc
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main-capability",
  "description": "ConvertIA main window — minimal offline file-converter surface",
  "windows": ["main"],
  "permissions": [
    "core:default",                       // base webview/window/event/path (incl. Channel)
    // — our own #[tauri::command]s C1..C13 need NO per-command permission entry: in
    //   Tauri v2, once a custom command is on the invoke_handler and this capability
    //   covers the "main" window, it is invokable. Per-command permission entries are
    //   ONLY required for PLUGIN commands (dialog/log/store). So we add NO C1..C13
    //   allow-entries here (adding them would be redundant, not load-bearing).
    // CAVEAT (load-bearing, verified vs Tauri v2 source `webview/mod.rs` +
    //   `acl/mod.rs::has_app_manifest`): a custom (app-own) command requires ACL/capability
    //   validation ONLY when one of: (1) it is a PLUGIN command, (2) the app has defined its
    //   own APP ACL MANIFEST (the `__app-acl__` key — emitted when the app declares per-command
    //   permissions for its own commands, the Tauri-encouraged production-hardening path,
    //   wired via build.rs `tauri_build` app-manifest/commands), or (3) the request comes from
    //   a REMOTE origin. v1 hits NONE: no app ACL manifest is defined (DEFAULT), and the
    //   WebView is local-only (no remote origin, §0.10 CSP). So C1..C13 need NO per-command
    //   entry HERE as the implemented v1 path. **If a future build opts INTO the app ACL
    //   manifest, each C1..C13 then needs an `allow-<cmd-name>` entry or it is silently
    //   DENIED** — do not add the opt-in without adding all C1..C13 allow-entries. (Remote
    //   origin never applies: ConvertIA serves only the bundled local app.)
    // C2a pick_for_intake / C2b pick_destination: BOTH native pickers are opened
    //   RUST-SIDE via DialogExt from their handlers `[DECIDED]` — so there is **NO
    //   `dialog:allow-open` grant**. The INTAKE picker (C2a) funnels picked paths
    //   straight into the C1 freeze and returns a CollectedSet, so intake paths never
    //   transit the untrusted WebView (mirrors the opener model). The DESTINATION
    //   picker (C2b) returns the chosen folder PathBuf to the WebView for C5 — that
    //   one WRITE-destination path does transit the WebView (acceptable: §0.11 T2,
    //   bounded by §2.1 non-destructive creates). A Rust-internal DialogExt call is
    //   not capability-gated either way.
    // file-system: the core does the FS work in Rust; the WEBVIEW gets NO fs plugin
    //   scope at all (no fs:default) — it cannot read/write files directly.
    // NO shell:allow-execute — engines spawn Rust-side only (§3.3.3 [DECIDED]); the
    //   WebView is granted no command-execute surface at all. (Removed deliberately;
    //   a raw Rust tokio::process spawn is not capability-gated, so no grant is
    //   needed, and granting one would only re-open the threat §3.3.3 closed.)
    //
    // NO opener:* grant on the WebView either `[DECIDED]`. The "open folder /
    //   open file" affordance (C9) and "open project page" (C10) are ConvertIA's
    //   OWN typed IPC commands; their Rust handlers call the opener plugin's
    //   `OpenerExt` (reveal/open/openUrl) INTERNALLY (§7.7.1). A Rust-internal
    //   `OpenerExt` call is NOT capability-gated (capabilities gate only what the
    //   WebView may invoke), so no `opener:allow-*` permission is required — and
    //   granting one would be the WRONG model here: a static `opener` path scope is
    //   an enforced OUTER bound applied BEFORE any Rust handler runs, so it can only
    //   FURTHER-RESTRICT, never widen. Since the §2.7 default writes output BESIDE
    //   the source (Desktop, USB, arbitrary project folders — routinely outside
    //   $DOWNLOAD/$DOCUMENT), a $DOWNLOAD/$DOCUMENT-scoped grant would SILENTLY
    //   BREAK the one-click open-folder/open-file DoD gate for the common case.
    //   The real, sufficient gate is the Rust-side RunResult-membership check
    //   (§7.7.3): C9 opens a path only if it is in the current run's recorded
    //   outputs (or their common root) — which works for arbitrary beside-source
    //   destinations. C10 is locked to the compiled-in project URL constant in Rust
    //   (no WebView-supplied URL). See §0.4.1 C9/C10, §7.7.2/§7.7.3.
    "log:default",                        // §7.5.1 JS→Rust log bridge (frontend errors → same local file)
    "store:default"                       // §7.4.2 the single settings.json prefs blob (theme + lastDestinationMode + verboseLog)
  ]
}
```

Notes / deliberate exclusions:

- **No `fs:` scope is granted to the WebView.** All filesystem access is Rust-side
  through `guarantees-fs`; the UI never reads or writes files. This is stronger
  than the SSOT minimum and shrinks the threat surface (§0.11).
- **No `http`/`fetch` permission, no updater plugin** → the WebView has **no
  network capability** (reinforces *offline*; §2.11, §7.6).
- **No `shell:allow-execute` at all `[DECIDED]`.** Engines are spawned **only by the
  Rust core** via `tokio::process` (path resolved through the Tauri PathResolver,
  §3.3.3), never from the WebView. There is therefore **no** shell-execute grant on
  the allowlist — the WebView cannot start an engine; the only way to begin a
  conversion is the typed C6 command the core validates against the registry and the
  frozen job. This is the §3.3.3 [DECIDED] resolution; the prior draft's
  `shell:allow-execute` block contradicted it and is removed (it was either dead
  surface-widening or implied a spawn path §1.7 rejects). The Tauri **opener** plugin
  is a *separate* plugin from shell-execute; the opener grants above do **not** grant
  command execution.
- **`opener` is NOT a WebView grant `[DECIDED]`.** C9 (open folder / open file) and
  C10 (open project page) are ConvertIA's own typed IPC commands; their Rust handlers
  call the opener plugin's `OpenerExt` (reveal / open-path / open-url) **internally**.
  A Rust-internal `OpenerExt` call is not capability-gated, so the manifest carries
  **no `opener:allow-*` permission**. The authoritative gate is Rust-side: C9 validates
  the requested path against the current `RunResult`'s recorded outputs (or their common
  root) before opening (§7.7.3 — works for arbitrary beside-source destinations, which a
  static `$DOWNLOAD/$DOCUMENT` scope could never cover), and C10 opens only the
  compiled-in canonical project URL (no WebView-supplied URL, §7.6). `reveal-item-in-dir`
  is the safer primary "open folder" affordance (it does not execute the file); open-path
  is secondary. (Rationale for dropping the static scope: a capability allow-list is an
  enforced **outer** bound applied **before** the Rust handler — it can only further-
  restrict, never widen — so a $DOWNLOAD/$DOCUMENT glob would silently break the
  beside-source open gate, not secure it.)
- **`log:default`** is on the allowlist because §7.5.1 ships a thin JS→Rust log
  bridge (frontend errors land in the same local-only file). It grants **no network**
  — the log sink is a local file; CSP still forbids remote origins.
- **`store:default`** is on the allowlist for the single `settings.json` prefs blob
  (§7.4.2: theme + lastDestinationMode + verboseLog). **`store:default` grants all store
  operations with no per-file scope** (it covers every store the plugin creates — there is
  no Tauri-native per-file scope, §7.4.2); ConvertIA limits itself to the one
  `settings.json` **by convention** (its only store call site), not by a permission scope.
  Both `log:` and `store:` are local-only and consistent with *offline / no
  system-pollution* (a single OS-config-dir file, no network).

**Content-Security-Policy (`tauri.conf.json → app.security.csp`)** — *recommended,
no remote origins (reinforces "no network"):*

```jsonc
"csp": {
  "default-src": "'self'",
  "script-src": "'self'",
  "style-src": "'self' 'unsafe-inline'",   // Tailwind/inline-style needs; tighten with nonces if feasible
  "img-src": "'self' data: blob:",         // app assets + generated previews/thumbnails as data/blob (NO asset: — v1 renders no user file from disk; §0.10 note)
  "font-src": "'self'",
  "connect-src": "'self' ipc: http://ipc.localhost",  // Tauri v2 IPC custom protocol ONLY — NO https/remote
  "media-src": "'self' blob:",             // generated content only (NO asset:)
  "object-src": "'none'",
  "base-uri": "'self'",
  "form-action": "'self'",                 // no form POST to a remote target
  "webrtc": "'block'",                     // best-effort: blocks RTCPeerConnection on Chromium/WebView2; likely a no-op on macOS WKWebView / Linux WebKitGTK (spec default 'allow')
  "frame-src": "'none'",
  "frame-ancestors": "'none'"              // no embedding of the app window in a frame (added P0 review r3; asserted by build-gates G47)
}
```

- **No remote origin appears anywhere** in the CSP — **no ordinary fetch/XHR/
  WebSocket/remote-subresource network is possible** from the WebView (the only
  `connect-src` is the Tauri IPC protocol; `form-action 'self'` blocks remote form
  POST; `webrtc 'block'` is **best-effort** — it blocks the RTCPeerConnection channel
  on Chromium/WebView2 but is **likely a no-op on macOS WKWebView and Linux WebKitGTK**
  (those engines default the directive to 'allow'), so it cannot be relied on
  cross-platform). CSP alone does **not** close every exotic side channel (DNS-prefetch,
  CSS-based timing, the WebRTC gap above), so the **load-bearing** cross-WebView
  offline enforcement is **§3.3.4 nothing-to-fetch** (the app opens no socket) + the
  **§2.11.4 packet-monitor release gate** (the actual proof; §2.12.3 engine-side OS
  network-deny is the **best-effort privilege-drop tier** `[DECIDED]` — defence-in-depth
  that degrades silently to the cheap tier, **not** the load-bearing guarantee). The CSP
  is the observable WebView-side form of
  *Local/private/offline* (verified in §2.11 / §6.4); the §2.11.4 packet gate is the
  load-bearing proof. **Accepted residual `[DECIDED]` (honest bound):** the `webrtc 'block'` no-op on 2
  of 3 WebView engines is an **explicitly-accepted residual** — even if a WKWebView/
  WebKitGTK WebRTC channel could be opened, the WebView has **no filesystem read access**
  (no `asset:`, no `fs:` plugin, it cannot read **file bytes** from disk). It does, however,
  hold **path STRINGS + conversion METADATA** — the §0.6 `DroppedItem.raw_path` basenames it
  derives the BatchSummary preview from, the C2b destination `PathBuf`, and per-item
  outcome/format data. So the **honest worst-case leak over an exotic WebRTC channel is
  filenames/paths + conversion metadata, NOT file contents** (a far smaller surface than "the
  disk"). The real bound is the **no-WebView-FS-read model + §3.3.4 nothing-to-fetch + the
  §2.11.4 packet gate**, not the CSP directive; the residual is bounded to path/metadata
  strings and is not chased with a per-engine workaround.
- **No `asset:` protocol.** `asset:` is dropped from `img-src`/`media-src`: v1 renders
  **no** user file from disk in the WebView (there is no preview feature in §05), it
  would contradict the no-WebView-FS model, and the asset protocol would additionally
  need `assetProtocol.enable` + a scope + an `asset.localhost` CSP host on Windows
  (none declared). `data:`/`blob:` remain for app-generated content only. A future
  in-WebView preview would be a `[DEFER]` that re-adds `asset:` with the required
  config.
- `style-src 'unsafe-inline'` is the one pragmatic loosening (Tailwind + React
  inline styles); tightening to nonces is a polish item, not a gate. (Note: the
  platform "no inline CSS" rule targets hand-authored stylesheets; framework-
  emitted styles under a locked CSP are the accepted exception here.)
- **Three by-construction release-hardening config keys are asserted absent/false
  `[DECIDED]`** (each is a real Tauri v2 `tauri.conf.json` knob that, if flipped, would
  widen §0.11 T2 silently — they are structurally enforced by build-gates **G47**, which
  already parses `tauri.conf.json`):
  - **`app.withGlobalTauri` MUST be absent/`false`** (the Tauri default). When `true` it
    injects the **full Tauri API onto `window.__TAURI__`**, so any XSS or supply-chained
    frontend dependency could invoke our IPC commands **directly from JavaScript** instead
    of only through the app's own React code — a direct T2 widening. v1 uses the `@tauri-apps/api`
    module imports, not the global, so the global is never needed.
  - **`app.security.dangerousDisableAssetCspModification` MUST be absent / `false` /
    empty-array.** It suppresses Tauri's built-in CSP modification for the listed directives;
    if set it could silently strip the enforcement of `script-src 'self'` / `connect-src`
    from the injected CSP layer **even when this §0.10 CSP object declares them** — defeating
    the offline CSP proof. The name carries `dangerous` for exactly this reason.
  - **`app.windows[].devtools` (and the bundle/release `devtools` feature) MUST NOT be
    enabled in the release/bundle profile** — devtools open in a shipped build is an
    inspection/injection surface on the untrusted WebView. (Debug builds may enable it; the
    release profile must not.)

**Status `[DECIDED]`.** The allowlist shape **and** its concrete contents are now
fixed: deny-by-default; **no** WebView FS; **no** network; **no `shell:allow-execute`**
(engines spawn Rust-side per §3.3.3); **no `opener:*` WebView grant** (C9/C10 are
ConvertIA's own commands whose Rust handlers call `OpenerExt` internally — not
capability-gated — and the real gate is the Rust-side §7.7.3 RunResult-membership
check, which works for arbitrary beside-source outputs a static scope could not);
**no `dialog:allow-open` WebView grant** `[DECIDED]` (both C2 pickers are opened
Rust-side via `DialogExt`: the **intake** picker C2a funnels picked paths into the C1
freeze and returns a `CollectedSet`, so **intake** paths never transit the WebView;
the **destination** picker C2b returns the chosen write-destination `PathBuf` to the
WebView for C5, which is acceptable per §0.11 T2). **Scope note — the "WebView never
sees raw FS paths" claim is precise, not absolute:** it holds for the *picker* intake
surface, but the **primary intake (drag-and-drop) structurally delivers raw paths to
the WebView** via Tauri's native `onDragDropEvent` Drop payload (§1.1/§5.4), and the
**OS launch-arg / `app://intake`** path emits `Vec<PathBuf>` to the WebView that it
echoes back to C1. The real mitigation is **not** "no path ever reaches the WebView"
but that the **core treats every WebView-supplied path (drop, launch-arg, and a C5
destination) as untrusted input re-validated at the §1.1 freeze / §2.3.3 write-target
check** (canonicalise / resolve-identity / existence / detection); the DialogExt
picker simply avoids *one extra* such surface and the `dialog:allow-open` grant.
`log:default` + `store:default` for the §7.5 local log
bridge and the §7.4 prefs blob. The image-core runs as a **separate image-worker
process** `[DECIDED]` (§0.7/§2.12/§3.5.5) — a raw Rust spawn, so it adds **no**
WebView capability regardless. The
former `[OPEN]` (shell scope WebView-exposed vs Rust-only) is **closed: Rust-only,
no shell grant** (§3.3.3). Cross-refs: §3.3.3 (spawn model), §7.4 (store), §7.5 (log),
§7.7 (opener scope it constrains).

---

## 0.11 Security model & threat-surface map

One assembled map. The pieces are **owned elsewhere**; this section's job is to
prove **coverage** — every threat class has a named owner and no class is orphaned.
The `SECURITY` policy (§6.8) references this map.

| # | Threat class | Vector | Owner (mechanism) | Status |
|---|---|---|---|---|
| T1 | **Untrusted decoder input** | A crafted/corrupt/malicious file (image bomb, malformed MP4, hostile SVG, macro-laden DOCX) exploits or hangs a decoder | **§2.12** decoder isolation (separate subprocess for **every** engine including the image core — the image-worker process `[DECIDED]` §0.7/§3.5.5; contained crash/hang/exploit fails one item) + **§1.7** invocation lifecycle (timeout/kill) + **§0.9** pool bounds + **§1.2** detection security note (first code on untrusted bytes). **v1 ships no rely-on-OS decode path**; any future rely-on-OS untrusted-decode must pass the **§3.4.4** re-evaluation gate before counting as T1-covered. | covered |
| T2 | **Malicious / compromised WebView content** | XSS-style injection or a supply-chained frontend dep tries to read the disk or call out | **§0.10** capability allowlist (no WebView `fs`, no network) + CSP (no remote origins, `object-src 'none'`) | covered |
| T2a | **WebView steers writes to an attacker-chosen path** | A compromised WebView supplies an arbitrary `DestinationChoice::ChosenRoot(PathBuf)` to C5/C6 (the destination is WebView-held, with no server-side store — §0.4.1 C6) to write outputs somewhere unexpected | **§2.1** writes are always **non-destructive creates** (never overwrite) + **§2.3.3** write-target link-safety (a chosen destination that resolves onto / inside a frozen source is rejected and diverted) + **§2.7** divert rules. A chosen destination is honoured only as a *write* location: it **cannot harm an original** (no-clobber + link-safe) and **cannot read anything** — so an arbitrary writable ChosenRoot is bounded harm (a converted copy lands in an odd-but-writable folder), accepted in v1. The C2b destination picker is Rust-opened, but C5/C6 still accept a WebView-supplied `ChosenRoot` string; the no-harm machinery — not path provenance — is the bound. | covered |
| T2b | **WebView re-submits an attacker-chosen SOURCE path** | On the idle launch/Open-with path the core emits `app://intake` carrying the full `Vec<PathBuf>` to the (untrusted) WebView, which echoes those paths back to **C1 `ingest_paths`** — a trust-boundary crossing (the WebView holds source paths it then re-submits). A compromised WebView could substitute an arbitrary readable path before re-submission. | **Accepted bounded harm (same posture as T2a).** The only harm a substituted source path can cause is "**convert an attacker-named readable file to an output beside it**" — it **cannot overwrite or harm any original** (§2.1 no-clobber + §2.3 link-safety bound the *write*) and produces only a converted copy. The bound is the **freeze-time §1.1 re-validation** (canonicalise / resolve-identity / existence / detection at the §2.4 freeze), **not** path provenance: every path C1 receives — regardless of whether it came from a native drop, the Rust picker, or a WebView `app://intake` echo — is re-validated at the freeze before any engine touches it. (The C2a **intake-picker** funnel keeps source paths Rust-side entirely; this T2b row covers only the launch-arg/`app://intake` echo, which is unavoidable because the OS hands the launch paths to the running instance and the idle UI drives C1.) | covered |
| T2c | **WebView plugin-write surface (`store:default` + `log:default`)** | The WebView is granted `store:default` (the 3-key prefs blob, §7.4.2) and `log:default` (§7.5) — the ONE place it can cause a *write*, so the "no WebView fs" claim in T2 is not absolute and must be named or it is an orphan class | **Bounded to the OS config dir, no user-file contents, no exfil `[DECIDED]`.** The store writes only the 3 fixed prefs keys (`theme`/`lastDestinationMode`/`verboseLog`) and the log writes only diagnostic lines — **never user file CONTENTS**, never to an arbitrary path: both are confined to `app_config_dir()` (`~/.config/dev.ne-ia.convertia/…`). The store **name is a compiled-in constant** (the WebView supplies no store filename), so it **cannot traverse out of `config_dir`** via a `../`-style name in the pinned `tauri-plugin-store` version (a §6.1.3/§0.10 assertion confirms the plugin version cannot escape `config_dir`; if a future plugin version ever could, the prefs writes move Rust-side). The worst-case harm is corrupting the local prefs/log (a clean reset recovers), never reading or exfiltrating user data — so this write surface is bounded and named, not orphaned. | covered |
| T3 | **Bundled-binary supply chain** | A tampered/backdoored engine binary ships in the build | **§3.8** engine pinning + **§6.2** integrity hashes + **§6.3** SBOM (every binary enumerated, verifiable). **Build-time** the pinned-checksum + SBOM gate catches a swapped engine; the trust anchor is the published **SHA256SUMS + minisign signature verified BEFORE first run (§6.2)**. **Runtime caveat:** the §7.2.3 startup check verifies engines against a hash manifest shipped **inside the same bundle**, so it detects **corruption/integrity** (truncation, AV-gutting, partial extract) but provides **no runtime tamper-resistance** — an attacker who can replace a binary can replace the in-bundle manifest too; runtime tamper detection is **out of scope** (unsigned portable build, SSOT). | covered (corruption/integrity only; runtime has no tamper-resistance — trust anchor is the §6.2 SHA256SUMS + minisign verified before first run) |
| T3a | **DLL/dylib/`.so` side-loading of a bundled codec shared object** | ConvertIA stages dynamically-loaded codec shared objects beside its engine executables (`libmp3lame.dll`/`libvorbis`/`libopus`/`libvpx` beside FFmpeg on Windows — §3.6.1 carve-out i; the image-worker codec stack as resources). A portable zip extracted into an attacker-controlled directory, or a directory pre-seeded with a matching-named malicious `.dll`, exploits the OS DLL/dylib search order so the engine subprocess loads the attacker's library | **Every staged `.dll`/`.dylib`/`.so` is individually enumerated in `engines.lock` with its SHA-256 (§3.7.2) and verified before staging (§6.1.3, the T3 checksum gate extended per-shared-object); the staging manifest-diff hard-fails on a staged shared object not matching its `engines.lock` row; a staging-time dynamic-dependency-closure check (`ldd`/`readelf` Linux · `otool -L` macOS · `dumpbin /dependents` Windows) asserts every non-system dependency resolves INSIDE the bundle; on Windows engines are spawned with a minimal explicit `PATH` (the bundle dir only) so the search starts inside the bundle, composing with the §3.5 loader-injection-var strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*` cleared). | covered |
| T4 | **Open-file launch of a fresh artifact** | C9 "open file" hands a just-written, possibly-still-untrusted output to an external app | **§7.7** open-file safety (reveal-in-folder, no auto-open, the artifact is *our* output not the untrusted source) + **§7.7.3** Rust-side `RunResult`-membership check (only a path that is a member of the current run's results may be opened). (Note: §0.10/§7.7.2 deliberately grant **no** `opener:*` path scope — beside-source outputs legitimately write outside `$DOWNLOAD`/`$DOCUMENT` — so the gate is the membership check, not a capability path-scope.) | covered |
| T5 | **Core panic / app fault** | A Rust panic, WebView load failure, missing/corrupt engine at startup, damaged bundle | **§2.13** app-level fault model (`catch_unwind` worker boundary, no-stack-trace surfacing) + **§7.2** startup faults + **§0.3.1** WebView-absent handling | covered |
| T6 | **Copyleft aggregation boundary** | Accidentally linking a GPL/LGPL engine into the MIT core (licence contamination) | **§3.6** copyleft isolation (separate invoked binaries, aggregation not linking) — architecturally enforced by the §0.3 subprocess model + §0.7 (engines are sidecars, never linked) | covered |
| T7 | **Path / link redirection** | A symlink/junction/alias makes an output resolve onto a source, or a TOCTOU race redirects the final write | **§2.3** resolved-identity & link safety + **§2.1** exclusive create-new-or-fail (the no-clobber guarantee is evaluated on the resolved real file) | covered |
| T8 | **Self-feeding / batch expansion** | Outputs written into a watched source folder get re-ingested, or a second instance's files appear mid-run | **§2.4** frozen source set + **§7.1** instance/run identity (per-run temp ownership, no cross-instance ingestion) | covered |
| T9a | **ConvertIA's own code exfiltrates user files** | The app itself (Rust core or WebView) tries to upload originals/results | **Structurally covered:** ConvertIA's own code **opens no socket** — no HTTP/updater plugin on the §0.10 allowlist, no `connect-src` to remote origins (CSP), `form-action 'self'`, no phone-home (**§7.6**). The only network is the user-initiated C10 open-project-page shell-out. Proven by the **§2.11.4** packet-monitor release gate (blocks release on any outbound packet) + **§2.11** offline invariant. | covered |
| T9b | **A bundled engine reaches out on hostile input** | A crafted dropped file makes a bundled engine (FFmpeg HLS/DASH/concat, pandoc include, LibreOffice remote/OLE link, **a crafted SVG's `<image href>`/XInclude**) open an outbound socket or read an out-of-input file at convert time (SSRF/LFR; e.g. CVE-2023-6605, librsvg CVE-2023-38633) | **Load-bearing argv/build controls (NOT the degradable OS sandbox), BOTH halves:** **§3.5.1** FFmpeg `-protocol_whitelist file,pipe` + network-disabled build (SSRF half) **and** concat `-safe 1` (never `-safe 0`, rejects absolute/`..` paths) + a curated demuxer set without the playlist/manifest dereferencing demuxers (absolute-file LFR half) — both asserted at **§6.1.3** (`ffmpeg -protocols` + `-demuxers`); **§3.5.4** pandoc `--sandbox`; **§3.5.2** LibreOffice profile-hardening (no link/OLE auto-update); **§3.5.5** SVG/librsvg control — load the SVG via `rsvg::Loader` with **NO base URL/`base_file`** so librsvg refuses **all** local `<image href>`/XInclude resolution by construction (no base URL = nothing to resolve against) and remote schemes regardless; calls librsvg directly (libvips `svgload` has no external-resource toggle); **no base-URL confinement is used** (supplying any base URL is what re-enables the CVE-2023-38633-class surface). Closes the SVG absolute-file LFR half, the librsvg analogue of the FFmpeg LFR control. Backed by the **§6.4.2** adversarial-egress / network-trigger case (zero egress AND no out-of-input file read on a network-trigger input), the **§6.1.3** SVG external-`<image href>` corpus assertion (no out-of-input bytes embedded), and proven again by the **§2.11.4** packet-monitor gate. **Defence-in-depth only (no longer load-bearing for either half):** **§2.12.3** engine-side OS network/FS restriction — the **best-effort privilege-drop tier** `[DECIDED]` (present where it works without install-time elevation, **degrades silently to the cheap tier** otherwise), so it is **not** the structural guarantee; the per-engine argv/build controls are. | covered |
| T10 | **Resource exhaustion / DoS-by-input** | A tiny SVG asked to render at 50 000 px, a 90-min→GIF, a thousands-file batch exhausting RAM/disk/handles | **§1.10** resource pre-flight & budgets + **§0.9** pool/handle bounds + the to-GIF guardrail (cross-category.md) | covered |
| T11 | **macOS engine-as-first-TCC-accessor (silent-deny)** `[DECIDED — P0 review r3]` | On macOS the source frequently sits in a TCC-protected location (Desktop/Documents/Downloads/removable). If a spawned engine were the FIRST process to touch such a path, a TCC responsible-process chain-break triggers an **invisible denial / wrong-process prompt** that defeats the conversion — and it is **silent on CI** (which runs from `TMPDIR`, where no TCC prompt fires), so a P4/P5 refactor that drops the pre-copy passes CI yet fails for real users on Desktop. **§3.5.0/§7.2.6 call this load-bearing for every macOS engine read.** | **§3.5.0 / §7.2.6 macOS TCC source staging:** the Rust core (which holds the TCC grant, having read the path at §1.1 freeze) copies the source into a **per-job kind-2 scratch path** (§2.14.2) **before** spawning, and hands the sidecar the **scratch path** — so the engine is **never the first process to touch a protected path**. Verifying gate: **G31** macOS sub-test (the Rust core, not the engine PID, is the first accessor; the engine receives a kind-2 scratch path) **and** a **G29** Semgrep rule (every `Command::new` in `crate::isolation` under `cfg(target_os="macos")` is preceded by the stage-for-TCC call). | covered |
| T12 | **Unsigned distribution / download-MITM** | An attacker tampers with the artifact / `SHA256SUMS` between our GitHub release and the user (binary code-signing is out of scope — no OS-level signature to verify) | **§6.2** minisign-signed `SHA256SUMS` + per-file SHA-256 + the published verify recipe + an **out-of-band pubkey-fingerprint** anchor (the in-repo pubkey TOFU is otherwise circular), verified BEFORE first run; the unsigned-build OS friction (SmartScreen/Gatekeeper) is documented at **§6.2.4** | covered (verify-before-run; no runtime tamper-resistance — accepted residual, SSOT *Out of Scope*) |

**No orphan classes.** Every box above points at a section that owns the
mechanism; this file invents none of them. If a new threat class is identified
during implementation it is added here with an owner before code lands (the map is
the coverage contract the §6.8 `SECURITY` policy points at).
