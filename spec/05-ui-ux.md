# 05 — UI / UX

> Frontend architecture and the concrete UI. Origin: SSOT *How It Feels to Use*,
> *Design Intent*, *It just works by default*, *For anyone — accessible*,
> *Modern, clean UI*, *English UI*.
>
> **Ownership reminder.** This file owns the **frontend**: React/TS/Tailwind
> structure, the screen-state machine, the component inventory, the design system,
> accessibility, the frontend half of the IPC async model, the About/legal-notices
> *presentation*, and the canonical keyboard map (§5.10). It **references, never
> restates**: the IPC contract (commands/events/payloads/cancel token) → **§0.4**;
> the type-sharing mechanism → **§0.4.5**; the pipeline / detection / batch rule /
> progress / cancel mechanism → **§01**; the hard guarantees and their
> outcome-strings (failure §2.8, lossy §2.9) → **§02**; the patent×platform target
> availability → **§3.4**; the third-party-licence / SBOM data → **§3.7**; the
> open-folder/open-file shell-out → **§7.7**; the Tauri capability allowlist + CSP
> → **§0.10**; instance/run identity → **§7.1**. Where this file *displays* a value
> another section *produces*, the producer is named inline.

---

## 5.1 Frontend stack & structure

### Stack `[DECIDED]`
React 19 + TypeScript (strict, **no `any`** per the platform rule) + Tailwind CSS,
built by Vite, running inside the Tauri WebView. Pinned versions are owned by
**§0.8** (not re-pinned here). The frontend is **UI only** — it holds no
conversion logic, no filesystem access, and no engine knowledge; every effectful
operation crosses the IPC boundary into the Rust core (§0.3 two-tier model). The
WebView runtime varies per OS (WebView2 / WKWebView / WebKitGTK, §0.3.1) — the
frontend therefore targets the **intersection** of those engines and avoids
bleeding-edge CSS/JS that drifts across them (rendering-drift testing is §6.4).

### Component / folder structure `[DECIDED]`
Frontend lives under the physical tree owned by **§0.7** (`src/`). Logical
grouping inside it:

```
src/
  main.tsx                     # React root mount, providers
  App.tsx                      # top-level screen-state router (§5.2 machine)
  ipc/                         # the ONLY module allowed to call invoke()/Channel/events
    commands.ts                # thin typed wrappers over §0.4 commands
    events.ts                  # progress Channel + event subscription helpers (§5.8)
    types.ts                   # generated/mirrored Rust↔TS types (mechanism §0.4.5)
  state/
    machine.ts                 # the screen-state machine (§5.2), reducer + guards
    store.ts                   # app store (choice below)
  components/
    DropZone.tsx               # §5.3
    BatchSummary.tsx
    FileList.tsx
    FormatPicker.tsx
    OptionsPanel.tsx
    AdvancedDrawer.tsx
    DestinationBar.tsx
    ProgressList.tsx
    ResultSummary.tsx
    OpenActions.tsx            # backed by §7.7
    RerunPrompt.tsx
    MixedDropRefusal.tsx
    UnsupportedNotice.tsx
    AboutDialog.tsx            # presents §3.7 data
    primitives/                # Button, Dialog, Drawer, Tile, ProgressBar, Note…
  design/
    tokens.css                 # CSS custom properties (§5.5)
    theme.ts                   # token typings, light/dark resolution
  a11y/
    announcer.ts               # ARIA-live announcement helper (§5.6)
    keymap.ts                  # the §5.10 accelerator table, single source
  strings/
    ui.ts                      # UI-chrome English strings (§5.7 ownership split)
```

**Hard rule:** only `src/ipc/**` imports `@tauri-apps/api`. Components and state
talk to a typed façade, so the IPC contract (§0.4) has exactly one consumer and
the "no direct `fetch`/raw-`invoke` in feature code" discipline (platform
auth-pattern analogue) is enforceable by lint.

### State management choice `[DECIDED — Zustand, recommendation]`
The app is a **single linear wizard with one batch in flight at a time** (the
SSOT batch rule allows only one source format per run; §1.3). That makes the
state small and mostly a finite-state-machine plus a per-item progress list.

- **Screen-flow state** → a hand-rolled **reducer-based finite-state machine**
  (`state/machine.ts`, plain TS, see §5.2). A typed discriminated-union `State`
  + `dispatch(action)` is preferred over a library (XState would work but is
  weight the SSOT *lightweight* principle doesn't justify for ~9 states).
- **Shared app store** → **Zustand** `[recommendation]`. Rationale: tiny, no
  provider boilerplate, ergonomic selectors (avoids needless re-render of a
  1000-row progress list), and trivially testable in Vitest. The store holds the
  machine state, the collected batch, the chosen target+options, the resolved
  destination preview, and the live progress map. Redux Toolkit is heavier than
  warranted; bare Context+useReducer re-renders too broadly for the large
  progress list (§1.10 virtualisation). **This is the one genuinely
  substitutable decision in 5.1** — flagged as a recommendation, not owner-level
  blocking; any equivalent minimal store is acceptable so long as it keeps the
  IPC-façade rule and selector-granularity.
- **No data-fetching/cache library** (React Query etc.) — there is no server; all
  data crosses IPC and is push-driven via a Channel (§5.8), not request/cache.

### Shared types with Rust
The Rust↔TS type bridge (so `Batch`, `DetectedFormat`, `ConversionJob`,
`Target`, `RunResult`, error/lossy payloads, the progress event union) are
**typed end-to-end with no `any`** — mechanism owned by **§0.4.5** (manual mirror
vs ts-rs/specta/tauri-specta; CI drift check in §06). This section **consumes**
those generated types in `src/ipc/types.ts` and re-decides nothing.

---

## 5.2 Screen states / flow — **owner of the frontend state machine**

The UI is a finite-state machine. Each state is a discriminated union variant
carrying exactly the data that state needs; transitions are driven by user
actions and by inbound IPC results/events (§5.8). The backend is the source of
truth for *facts* (what was detected, what targets exist, where output will go,
per-item outcome); the machine only sequences the user through them.

### States (enumerated)

| # | State | Entered when | Primary content | Exits to |
|---|-------|--------------|-----------------|----------|
| 1 | `Idle` | app start; after "convert more"; after a refused/unsupported drop is dismissed | drop-or-browse invitation; "all conversion happens locally, on your machine" reassurance; no setup, no fields | drop/pick → `Collecting` |
| 2 | `Collecting` | a drop/pick/launch-arg handoff is accepted; backend is freezing the set + recursing folders + detecting (§1.1/§1.2) | indeterminate-OK "looking at your files…" *only for the brief collect step* (NOT the convert step) + a cancel-collect affordance | → `Confirm` \| `MixedDropRefusal` \| `Unsupported` |
| 3 | `Confirm` (collected/confirm gate) | backend returns a single-format collected summary (§1.4) | "**N JPG files**" (detected format + count); for recursive folder drops, the collected count is the whole point of this gate | confirm → `Targets`; cancel → `Idle` |
| 4 | `Targets` (targets + options) | user confirms the batch | FormatPicker (target tiles, one **pre-highlighted default** per §1.5/04-matrices), contextual basic options, **Advanced options** drawer (§5.3), passive **lossy note** beside the chosen target (§2.9) | pick target → reveal/refresh `DestinationBar` (same state); proceed → `Destination`-confirmed (folded) or directly to the convert gate |
| 5 | `Destination` (destination preview — folded into the Targets screen) | always shown **before** convert (SSOT *Output lands somewhere obvious*) | the "**will save to …**" line (per §1.8/§2.7 plan: beside each source by default, divert noted), **Change destination** button (opens dialog, §7.7), the **Convert** button | Convert → `Rerun?` decision (backend §2.5) → `Converting`; back → `Targets` |
| 6 | `RerunPrompt` (interstitial) | backend §2.5 flags equivalent output already exists (same resolved source + target + effective settings) | **one batch-level** prompt: *"You already converted these with the same settings."* — **Skip (default)** / **Make a fresh copy** | choose → `Converting`; cancel → back to `Destination` |
| 7 | `Converting` (progress) | convert command accepted | **per-item** real progress (not a spinner) + **aggregate batch** bar; current-item label; **Cancel** button | all items terminal → `Summary`; cancel → confirmed-cancel round-trip (§5.8) → `Summary` (partial) |
| 8 | `Summary` | every job reached a terminal state (§1.9) | per-item success/fail with reason (strings §2.8), output→source mapping (§1.12), **Open folder** / **Open file** (OpenActions, §7.7); a **fully-failed** batch is rendered as a clear failure banner, never a quiet "done" | "Convert more" → `Idle`; Open actions stay available |
| 9 | `MixedDropRefusal` | the drop/folder contained >1 source format (§1.3 pre-flight) | **hard refusal**, not a partial convert: lists the formats found + counts ("Found 30 JPG, 12 PNG, 3 PDF"), asks to **re-drop a single format**; explicitly **no** "just convert the JPGs" affordance in v1 (parked) | dismiss / re-drop → `Idle`/`Collecting` |
| 10 | `Unsupported` / `Unreadable` | detection says *real but unsupported type* or *uncertain/conflicting* (§1.2), or every collected item was unreadable/gone | plain message: *"Can't convert this type — detected: X"* or *"Couldn't tell what this file is"*; never an empty target list, never a hang | dismiss → `Idle` |

> **Mid-run skip vs pre-flight refusal — keep distinct (SSOT *Fail clearly*).**
> `MixedDropRefusal` (state 9) and `Unsupported` (state 10) are **pre-flight** —
> nothing is converted. A single bad/unreadable/too-big item discovered **during**
> `Converting` is **not** a state change: it surfaces as that item's terminal
> `Failed`/`Skipped` row in `ProgressList` and carries into `Summary`; the batch
> keeps going (§1.9/§2.8). Two visually similar "something's wrong" outcomes, two
> different mechanisms — the UI must not conflate them.

### State diagram

```
            ┌───────────────────────────── "convert more" ─────────────────────────────┐
            ▼                                                                            │
        ┌────────┐ drop / pick / launch-arg     ┌────────────┐                          │
        │  Idle  │ ───────────────────────────▶ │ Collecting │                          │
        └────────┘                              └─────┬──────┘                          │
            ▲  ▲                                      │ backend result                  │
            │  │                ┌─────────────────────┼──────────────────────┐          │
            │  │                ▼                     ▼                      ▼          │
            │  │        ┌──────────────┐       ┌────────────┐        ┌─────────────┐    │
            │  └────────│ MixedDrop    │       │  Confirm   │        │ Unsupported │    │
            │  dismiss  │ Refusal (9)  │       │  gate (3)  │        │ /Unreadable │────┘
            │           └──────────────┘       └─────┬──────┘        │   (10)      │ dismiss
            │                                        │ confirm       └─────────────┘
            │                                        ▼
            │                              ┌───────────────────────┐
            │                              │ Targets + Options (4)  │◀──┐ change target/opts
            │                              │  + Destination preview │   │ (in-place)
            │                              │  (5, folded in)        │───┘
            │                              └───────────┬────────────┘
            │                                          │ Convert
            │                              §2.5 equivalent-output?
            │                              ┌───────────┴───────────┐
            │                          yes │                       │ no
            │                              ▼                       │
            │                        ┌───────────┐                 │
            │                        │ Rerun (6) │ skip/fresh ─────┤
            │                        └───────────┘                 │
            │                                                      ▼
            │                                            ┌───────────────┐ cancel (confirmed
            │                                            │ Converting (7)│  round-trip §5.8)
            │                                            └──────┬────────┘
            │                                                   │ all jobs terminal
            │                                                   ▼
            │                                            ┌───────────────┐
            └─────────────────── "convert more" ─────────│  Summary (8)  │
                                                         └───────────────┘
```

### Patent-gapped / unavailable target rendering `[DECIDED — disabled-with-note, recommendation]`
A target that the **§3.4 format×platform matrix** marks *unavailable on this
platform* (e.g. HEIC encode where no redistributable HEVC encoder ships) is
shown in the FormatPicker as a **disabled tile with a one-line reason note**
("HEIC isn't available on this system"), **not silently omitted** —
recommendation, satisfying the SSOT *one product / honestly-surfaced exception*
(§9 first exception): the user sees the format *exists* and learns *why* it's
out, rather than wondering whether ConvertIA is incomplete. The availability flag
itself is **sourced from §3.4** via a backend capability query (§0.4 / §3.2
capability declaration) — the frontend never hardcodes a platform matrix.
*(Alternative — omit entirely — is the fallback if a disabled tile tests as
confusing in the §9 usability walkthrough; the consistency owner is §3.4, this
section only renders what the capability query returns.)*

---

## 5.3 Component inventory

Each component is presentational + wired to the store/machine; only `src/ipc/**`
crosses the boundary. Per-component keyboard behaviour **references §5.10**, not
restated per component.

| Component | Role | Key states/props | Notes / cross-refs |
|-----------|------|------------------|--------------------|
| **DropZone** | the primary intake surface + click-to-browse | `dragActive`, `disabled`-while-converting | native file-drop via §5.4; click opens picker (dialog plugin, §0.10 scope); the only element present in `Idle` besides the reassurance line |
| **BatchSummary** | the confirm-gate card | `detectedFormat`, `count`, `sampleNames?` | data from §1.4 collected-summary payload; the mandatory pre-convert gate (state 3) |
| **FileList** | optional expandable list of collected items | virtualised (§1.10) for thousands of files | read-only in v1 (no per-item target / no per-item deselect — both out of v1) |
| **FormatPicker** | target tiles for the detected source | `targets[]`, `default`, `selected`, per-tile `disabledReason?` | one pre-highlighted default (§1.5); cross-category outputs (extract-audio / to-GIF) appear as extra tiles of a video source (cross-category.md); disabled tiles per §3.4 (§5.2) |
| **OptionsPanel** | the few **basic** contextual settings for the chosen target | option descriptors (§1.6 generic model); values & defaults from 04 | e.g. JPG quality slider, GIF fps/width — **descriptors come from the backend** (§1.6), UI just renders the declared widget type |
| **AdvancedDrawer** | collapsed-by-default drawer for niche options | `open` | keeps the default view clean (SSOT How It Feels 5); never gates conversion |
| **DestinationBar** | the "will save to …" line + Change button | `plan` (destination preview), `diverted?` | **always visible before Convert** (state 5); shows per-location divert note (§2.7); Change → dialog (§7.7) |
| **ProgressList** | per-item rows + aggregate bar | `Map<JobId, JobProgress>`, `batchPct`, `currentItem` | real determinate progress (§1.11); virtualised for large batches; rows transition to terminal `Succeeded`/`Failed`/`Cancelled`/`Skipped` |
| **ResultSummary** | end-of-batch outcome | `RunResult` (§1.12) | success/fail counts, per-item reason (§2.8 strings), output→source map; fully-failed banner |
| **OpenActions** | open-folder / open-file buttons | `folderPath`, `filePath?` | **backed by §7.7** (the only OS shell-out); "open folder" opens the common root (§2.7) |
| **RerunPrompt** | the §2.5 interstitial | `equivalentCount`, default=Skip | one batch-level prompt, skip-default / fresh-copy (state 6) |
| **MixedDropRefusal** | pre-flight hard refusal | `formatsFound[]` with counts | state 9; no subset-convert affordance in v1 |
| **UnsupportedNotice** | unsupported / uncertain / all-unreadable | `detected?`, `reason` | state 10; plain language, no stack trace |
| **AboutDialog** | About + legal-notices | `licenseData` (from §3.7), `version` (§7.6) | presentation only — §5.9 |
| **Note** (primitive) | the passive lossy/divert/animation inline note | `kind`, `text` (string from §2.9) | calm, passive, never a blocking "I understand" dialog (SSOT *Fail clearly*) |
| **primitives/** | Button, Dialog, Drawer, Tile, ProgressBar, ProgressRing, Spinner, Banner, Toast? | — | the design-system building blocks (§5.5); a determinate ProgressBar is mandatory, an indeterminate Spinner is allowed **only** for the brief `Collecting` step |

---

## 5.4 Drag-and-drop & input — **the native file-drop boundary (a §0.4 fact)**

### Native file-drop (the load-bearing constraint)
**HTML5 drag-and-drop inside a Tauri WebView does not expose real filesystem
paths** (this is the §0.4 boundary fact). Intake therefore uses **Tauri's native
drag-drop event**, not the DOM `drop` event:

```ts
// src/ipc/events.ts — the ONLY place this is wired
import { getCurrentWebview } from '@tauri-apps/api/webview';

const unlisten = await getCurrentWebview().onDragDropEvent((e) => {
  switch (e.payload.type) {
    case 'enter':
    case 'over':  setDragActive(true); break;        // visual affordance only
    case 'leave': setDragActive(false); break;
    case 'drop':  setDragActive(false);
                  // e.payload.paths: absolute path strings (files AND folders)
                  ipc.ingestPaths(e.payload.paths);  // → §0.4 C1 ingest_paths
                  break;
  }
});
```

- `event.payload.paths` is an **array of absolute path strings** (not DOM `File`
  objects). Folders arrive as a directory path; **folder recursion runs in Rust**
  (§1.1) — the WebView cannot and must not enumerate a directory.
- The window must have `dragDropEnabled: true` (Tauri default) for the native
  event to fire; the DOM-level DnD that *would* hijack it is left disabled.
- **Known gotchas to handle (§6.4 test items):** native drag-drop events can
  duplicate / report differing webview ids on some platforms, and path payloads
  have shifted between Tauri patch versions — `src/ipc/events.ts` de-duplicates by
  the `paths` set per drop and treats the backend's frozen-set de-dup (§2.4) as
  the authority; the frontend never assumes the path list is unique or canonical.
- The drop only *hands paths to the backend*; the **frozen source set** (§2.4),
  detection (§1.2) and grouping (§1.3) all happen Rust-side. The UI transitions
  `Idle → Collecting` on `drop` and waits for the backend's collected summary.

### File picker (parity path)
Click on the DropZone (or the **O** accelerator, §5.10) opens the OS file dialog
via the Tauri **dialog plugin** (`open({ multiple: true, directory: false })`
for files; a separate "choose folder" affordance uses `directory: true`). The
dialog returns absolute paths that flow into the **same** `ingest_paths` (C1) entry
point as drop — one intake funnel (§1.1). Dialog/opener capability scope is owned
by **§0.10**; this section only invokes within that allowlist.

### Keyboard parity
Every result reachable by drop/pick is reachable by keyboard alone (SSOT DoD
gate). The DropZone is a focusable, `role="button"` element; **Enter/Space**
activates the picker; the full accelerator map is **§5.10**. There is no
keyboard-only dead end in the flow.

### Launch-time intake
Paths can also arrive via OS launch entry points (Open-with / argv / macOS
open-doc) — posture owned by **§7.8**; they feed the **same** `ingest_paths` (C1)
funnel, so the UI handles a launch-with-files identically to a drop (machine
enters `Collecting` at startup instead of `Idle`).

---

## 5.5 Design system

### Intent
"**Modern > plain**" (SSOT *Design Intent*): uncluttered, contemporary, a little
eye candy — never busy. Visual polish is **iterative and never release-blocking**
(SSOT §9 *Not a gate*); the *structure* and tokens here are the contract, the
exact palette is a placeholder the owner finalises (SSOT *Design Intent*: logo,
colours, branding are placeholders for now).

### Tokens `[DECIDED — token contract; values are placeholders]`
Design tokens are CSS custom properties in `design/tokens.css`, surfaced to
Tailwind via the theme config so components use semantic Tailwind classes, not
raw hex. Token groups:

- **Colour (semantic, not literal):** `--bg`, `--surface`, `--surface-raised`,
  `--border`, `--text`, `--text-muted`, `--accent` (the one brand accent),
  `--accent-contrast`, plus state colours `--success`, `--warn`, `--danger`,
  `--info`. Lossy/divert notes use `--info`/`--text-muted` (calm, **not**
  `--danger` — predictable loss is not an error). Failures use `--danger`.
- **Spacing scale:** 4-px base (`--space-1`=4 … `--space-8`=32) — Tailwind's
  default scale, kept.
- **Radius:** `--radius-sm/md/lg` (cards, tiles, the DropZone — generous rounding
  for the modern feel).
- **Typography:** a single clean UI sans (system stack +
  bundled-offline fallback so it renders identically with **zero network**, per
  the offline invariant §2.11); sizes `--text-xs … --text-2xl`; line-heights;
  weight `regular/medium/semibold`. **Readable contrast & text sizes are a DoD
  accessibility gate** (§5.6), so the *minimum* body size and contrast ratios are
  fixed, not placeholder.
- **Elevation/shadow, motion** tokens (below).

### Light / dark `[DECIDED — both, follow OS, recommendation]`
Support **light and dark**, defaulting to the **OS preference**
(`prefers-color-scheme`), resolved into the colour tokens at the root. Whether the
*chosen* theme persists across launches is a **persistence** question owned by
**§7.4** (`[OPEN]` there: v1 may persist nothing); if §7.4 lands on "persist
nothing", the theme simply follows the OS each launch — acceptable and consistent
with *portable / no system pollution*. No in-app theme toggle is required for v1
(following the OS is enough); a toggle is a cheap addition if §7.4 allows
persistence.

### Motion / eye-candy budget `[DECIDED]`
"A bit of eye candy is welcome" (SSOT) but **restrained and accessible**:
- Subtle transitions on the DropZone (lift/glow on `dragActive`), tile selection,
  drawer open/close, and state transitions (≤200 ms, ease-out).
- The progress bar animates smoothly between real values — **never** a fake/
  indeterminate crawl on the convert step (SSOT *Visible progress* demands a real
  bar so a long single conversion reads as *working, not hung*). An indeterminate
  spinner is permitted **only** for the brief `Collecting` step.
- **`prefers-reduced-motion` is honoured** (§5.6): all non-essential animation is
  disabled/reduced; progress still updates its value, just without easing.

### Loading / progress chrome
- Determinate `ProgressBar` (per-item) + aggregate batch bar in `Converting`.
- `Collecting` may show an indeterminate indicator + a cancel-collect control.
- No modal blocking overlay during conversion — the window stays responsive
  (SSOT *stays responsive regardless of batch or file size*), Cancel always live.

### Ne-IA logo placeholder `[DECIDED — placeholder]`
The **Ne-IA logo** appears as branding (header/About) per SSOT *Design Intent*.
It ships as a **bundled local asset** (offline; no CDN) behind a single
`<BrandLogo>` primitive reading a placeholder SVG, so the owner can swap the final
mark without touching layout. The logo and "ConvertIA"/"Ne-IA" names are **not**
under the MIT grant (SSOT *Trademark*) — the placeholder is a stand-in only.

---

## 5.6 Accessibility — **a v1 ship gate, not polish**

SSOT *For anyone — accessible* and the §9 DoD gate ("basic accessibility works:
keyboard path + readable contrast/sizes") make this **release-blocking**, on par
with no-harm. Concrete requirements:

- **Full keyboard operability.** Every action — open picker, confirm batch, pick
  target, open Advanced, change destination, convert, cancel, open folder/file,
  dismiss a refusal, answer the re-run prompt — is reachable and operable by
  keyboard alone (map §5.10). No mouse-only affordance exists. The drop area has a
  keyboard-equivalent (the picker).
- **Logical focus order** following the visual wizard order; focus is **moved to
  the new primary element on each state transition** (e.g. to the Convert button
  when the destination is shown, to the first failed row in `Summary`), and
  **trapped inside modals** (RerunPrompt, AboutDialog, MixedDropRefusal) with
  **Esc** to close (§5.10) and focus **restored** to the trigger on close.
- **Contrast & text size.** Body text and interactive elements meet **WCAG 2.1 AA
  contrast (≥4.5:1 text, ≥3:1 large text / UI)** against both themes; the minimum
  body size and the token scale (§5.5) respect OS text-scaling; nothing critical
  is conveyed by **colour alone** (the lossy note has text + icon, failures have a
  label not just red).
- **Screen-reader announcements** via an ARIA-live region (`a11y/announcer.ts`):
  - `Collecting`/`Confirm`: announce the collected summary ("48 JPG files
    found").
  - `Converting`: announce **batch milestones** (start, each item complete /
    failed *throttled* to avoid a 1000-item flood — e.g. every N% or on each
    failure), **not** every progress tick. The per-item bar carries
    `aria-valuenow`.
  - `Summary`: announce the outcome ("42 succeeded, 6 failed").
  - Errors/refusals are announced **assertively**; lossy/divert notes
    **politely** (they are calm, not alarms).
- **Semantics:** target tiles are a labelled radio-group (one selectable default);
  the DropZone is `role="button"`; the lossy note is associated with its target
  via `aria-describedby`; disabled (patent-gapped) tiles use `aria-disabled` +
  the reason text, never just visual dimming.
- The §9 **non-developer walkthrough per platform** is the validation that this is
  *actually* usable, not just spec-compliant.

---

## 5.7 Surfacing guarantees in the UI

This section renders the guarantees §02 implements; **conversion-outcome strings
(failure §2.8, lossy §2.9) are owned by §02** and pulled in verbatim — the UI
must not paraphrase them. **UI-chrome strings** (empty-state copy, confirm-gate
labels, button text, About text, the mixed-drop refusal phrasing) are owned
**here** (in `strings/ui.ts`) and share the same future-localization boundary
(§02 note). English only (SSOT *English UI*; localization parked).

| SSOT guarantee | How it shows in the UI | String owner |
|----------------|------------------------|--------------|
| **Predictable lossy** (SSOT *Fail clearly*) | a **passive inline `Note`** beside the chosen target the moment a lossy target is selected ("text only — layout and images are dropped"); shown **once**, calm, **never** a blocking "I understand" dialog or per-conversion nag; only for genuinely predictable loss | §2.9 |
| **Fail clearly** (per item) | the item's `ProgressList` row → terminal `Failed` with a plain reason; carried into `Summary`; batch continues; **no stack traces** (§2.13) | §2.8 |
| **Pre-flight refusal** (mixed drop) | `MixedDropRefusal` state (9) — distinct from a mid-run skip | here (chrome) |
| **Unsupported / uncertain** | `UnsupportedNotice` (10): "can't convert this type — detected: X" / "couldn't tell what this is" — never an empty target list, never an apparent hang | §2.8 / here |
| **Destination before convert** | `DestinationBar` "will save to …" is **always visible before** the Convert button is reachable (SSOT *Output lands somewhere obvious*); per-location divert noted | here (chrome), plan from §1.8/§2.7 |
| **Re-run / equivalent output** | `RerunPrompt` (6): one batch-level prompt, Skip default / fresh copy | §2.5 (logic), here (chrome) |
| **No-harm / atomicity** | invisible by design — the UI never offers an "overwrite" choice; collisions are silent next-free-variant (§2.2); only the *equivalent-output* re-run gets a prompt | §02 |
| **Cleanup couldn't complete** | if the backend reports residue (§2.6), the item is shown as **not a clean success** with where residue remains — never a green "done" | §2.6/§2.8 |
| **Fully-failed batch** | `Summary` renders a clear **failure** banner, never a quiet finish (SSOT *Fail clearly*) | here + §1.12 |
| **Offline / privacy** | the `Idle` reassurance line "all conversion happens locally, on your machine"; the About screen restates the offline + cloud-sync caveat (§2.11) | here / §5.9 |

**No blocking dialogs principle.** The only modal interruptions in the whole flow
are the **RerunPrompt**, the **MixedDropRefusal**, the **UnsupportedNotice**, and
the **AboutDialog** — each a deliberate decision point or dismissible info, never
a per-file nag. Lossy notes and divert notes are **non-modal** passive `Note`s.

---

## 5.8 IPC integration & frontend async model

The **command/event contract is owned entirely by §0.4** (every command name,
request/response payload, error shape, cancellation token, and every event/
Channel and its payload). This section defines only the **frontend async
behaviour** that *consumes* that contract. Nothing here re-declares a command
name or payload shape — `src/ipc/commands.ts` and `src/ipc/events.ts` are the
typed wrappers; feature code calls those.

### Command/response model
- All effectful operations are **`invoke()` calls** into the Rust core, awaited as
  Promises, typed via §0.4.5 generated types (no `any`). Conceptual calls (names
  defined in §0.4, not invented here): `ingest_paths` (C1), `get_targets` (C3),
  `plan_output` (C4), `start_conversion` (C6), `set_destination` (C5),
  `cancel_run` (C7), `open_path` (C9, via §7.7). The frontend treats these as
  opaque typed RPCs.
- Long-running work (the conversion run) must **not** block on a single Promise
  resolving at the end (Cloudflare-100s-style hangs don't apply locally, but a
  60-minute batch resolving one Promise at the end gives no progress) — instead
  the **`start_conversion` command returns quickly** and progress flows over a
  **Channel** (below). The pattern mirrors the platform's "respond immediately,
  stream/poll the rest" posture.

### Progress subscription lifecycle (Channel)
Per-item and batch progress stream from Rust via a **`tauri::ipc::Channel`**
(ordered delivery — correct for sequential progress) passed *into* the
`start_conversion` command. The payload is a **discriminated union** (the exact
variants/fields owned by §0.4; illustrative shape):

```ts
// shape OWNED by §0.4 — shown to fix the frontend handling, not to re-decide it
import { invoke, Channel } from '@tauri-apps/api/core';

type ConvertEvent =
  | { event: 'runStarted';  data: { runId: RunId; total: number } }
  | { event: 'itemStarted'; data: { jobId: JobId } }
  | { event: 'itemProgress';data: { jobId: JobId; pct: number } }
  | { event: 'itemDone';    data: { jobId: JobId; outcome: JobOutcome } } // Succeeded|Failed|Skipped|Cancelled (+reason §2.8)
  | { event: 'runFinished'; data: { result: RunResultSummary } };

async function startRun(plan: StartArgs): Promise<void> {
  const ch = new Channel<ConvertEvent>();
  ch.onmessage = (m) => store.applyConvertEvent(m);  // updates ProgressList/Summary
  await invoke('start_conversion', { args: plan, onEvent: ch }); // command name per §0.4
}
```

- **Lifecycle:** the Channel is created when entering `Converting`, its
  `onmessage` reduces into the store's progress map; on `runFinished` (or on a
  confirmed cancel) the machine moves to `Summary` and the Channel is dropped.
- **Throttling for large batches:** the UI coalesces high-frequency
  `itemProgress` ticks into animation-frame updates and relies on store-selector
  granularity so a 1000-row `ProgressList` (virtualised, §1.10) doesn't re-render
  per tick. SR announcements are throttled separately (§5.6).
- **Run/instance identity** (`runId`, single-vs-multi-instance) is owned by **§7.1**
  — the frontend just carries the `runId` the backend assigned.

### Cancellation — optimistic vs confirmed round-trip
SSOT *Visible progress, cancellable* + the cancel **mechanism owned by §1.7**
(process-group kill, no-partial-leftover) means cancel is **not** instant and
must be honest:

1. User hits Cancel (button or **Esc**, §5.10) in `Converting`.
2. UI **optimistically** flips to a "Cancelling…" affordance and **disables**
   Cancel (no double-cancel), but does **not** fabricate a finished state.
3. Frontend calls the **cancel command** (name §0.4) with the `runId` /
   cancellation token (§0.4 token shape).
4. The backend stops the queue, kills the in-flight engine (§1.7), cleans the
   in-progress temp (§2.6), and emits terminal `itemDone(Cancelled)` for the
   stopped item + `runFinished` over the **same Channel**.
5. Only on that **confirmed** `runFinished` does the UI move to `Summary`
   (partial): items already finished are **kept** (SSOT), the in-progress one is
   shown `Cancelled`, no partial leftover. The UI never claims "cancelled" before
   the backend confirms the kill+cleanup landed.

### Backend disconnect / panic / fault handling
Pairs with **§2.13** (app-level fault model) and **§0.9/§1.11** (concurrency/
progress). Frontend behaviour:
- **A single item's engine crash/hang** arrives as a normal `itemDone(Failed,
  reason)` (§2.13 catches the worker panic and reports it as a clean per-item
  failure) — the UI renders the failed row and continues; **not** an app-level
  event.
- **Channel goes silent / command Promise rejects unexpectedly** (core panic, IPC
  drop): the UI shows a **plain app-level fault** message (no stack trace, §2.13)
  in place of `Summary` — "Something went wrong and the conversion stopped." — and
  offers "Start over" → `Idle`. It does **not** invent per-item outcomes for items
  it never heard back about.
- **WebView/startup faults** (missing engine binary, damaged bundle, WebView fails
  to load) are **startup** concerns owned by **§7.2/§2.13** — outside this state
  machine (the UI may never even mount). This section only handles faults that
  occur **after** the UI is live.

---

## 5.9 Branding / About

The **AboutDialog** is a static in-app **About / legal-notices** screen
(SSOT *Design Intent*). It **presents** data produced elsewhere; it generates
nothing.

- **Third-party licences / NOTICE:** the bundled list of every engine's
  licence/attribution is **generated by §3.7** (NOTICE / third-party-licenses,
  backed by the SBOM). This section **displays** it (scrollable list:
  engine name → licence → notice text; copyleft engines flagged per §3.6 with the
  written-offer-of-source pointer). A **missing attribution is release-blocking**
  (SSOT §9), so the About screen rendering the §3.7 data correctly is part of that
  gate. The data ships as a **bundled offline asset** (no fetch).
- **Version:** the current app version is shown here; the **no-phone-home / no
  auto-update** posture and the user-initiated pointer to the canonical GitHub
  Releases page are owned by **§7.6** — About may render a *user-initiated* "open
  Releases page" link (the only permitted network, via §7.7), never an automatic
  check.
- **As-is / no-warranty + best-effort-security + cloud-sync caveat:** the About
  screen restates the SSOT *License & Openness* as-is/no-warranty notice, the
  best-effort security posture, and the *Local, private & offline* cloud-sync
  caveat (your own OneDrive/iCloud/Dropbox may sync originals/results — ConvertIA
  neither causes nor prevents it). Privacy/offline invariant text aligns with
  §2.11.
- **Branding:** Ne-IA logo (placeholder, §5.5) + "ConvertIA" name. Logo and names
  are **not** MIT-granted (SSOT *Trademark*); the About screen is where credits +
  third-party-licenses live (SSOT: no operated service → no web-style legal-notice
  obligation, so this in-app screen is the home).
- **Opening:** reachable from a header/menu affordance and the **F1 / ?**
  accelerator (§5.10); a modal dialog with focus-trap + **Esc** to close (§5.6).

---

## 5.10 Keyboard interaction model & shortcut map — **canonical accelerator map**

This is the **single source** of accelerators; per-component sections only
reference it (`a11y/keymap.ts`). It satisfies the SSOT §9 DoD gate
"drag/drop + picker + keyboard reach the same result." Modifier shown as
**Ctrl** on Windows/Linux and **Cmd (⌘)** on macOS (`CmdOrCtrl`).

### Global / context-aware accelerators

| Action | Accelerator | Available in | Notes |
|--------|-------------|--------------|-------|
| **Open file picker** | **Ctrl/⌘ + O**, or **Enter/Space** on focused DropZone | `Idle` (and `Summary` "convert more" returns to Idle) | parity with drop (§5.4); the picker is the keyboard equivalent of dropping |
| **Choose folder** | **Ctrl/⌘ + Shift + O** | `Idle` | directory-mode dialog (§5.4) |
| **Confirm batch** (proceed past the collected-summary gate) | **Enter** | `Confirm` (3) | the gate's primary action; **Esc** cancels back to `Idle` |
| **Select target tile** | **Arrow keys** within the radio-group; **Enter/Space** selects | `Targets` (4) | tiles are one radio-group; the pre-highlighted default is pre-focused |
| **Toggle Advanced options** | **Ctrl/⌘ + .** (period) | `Targets` (4) | opens/closes `AdvancedDrawer` |
| **Change destination** | **Ctrl/⌘ + D** | `Targets`/`Destination` (4/5) | opens the directory dialog (§7.7/§0.10) |
| **Convert** (start the run) | **Ctrl/⌘ + Enter** | `Targets`/`Destination`, only once a destination is shown | the primary action; never reachable before the destination preview exists |
| **Cancel conversion** | **Esc** | `Converting` (7) | triggers the **confirmed** cancel round-trip (§5.8); first Esc requests cancel, does not fabricate completion |
| **Open output folder** | **Ctrl/⌘ + Shift + F** | `Summary` (8) | OpenActions → §7.7 (common root, §2.7) |
| **Open output file** (single-result runs) | **Ctrl/⌘ + Shift + Enter** | `Summary` (8), when exactly one output | OpenActions → §7.7 |
| **Convert more / start over** | **Ctrl/⌘ + N** | `Summary` (8), app-fault screen | returns to `Idle` |
| **About / legal-notices** | **F1** (and **?** where no text field is focused) | any | opens `AboutDialog` (§5.9) |
| **Dismiss / close** any modal or notice | **Esc** | RerunPrompt, MixedDropRefusal, UnsupportedNotice, AboutDialog | closes + restores focus to trigger (§5.6) |

### Esc / Enter semantics on the decision gates (explicit)

| Gate | **Enter** | **Esc** |
|------|-----------|---------|
| **Confirm gate** (3) | proceed to Targets | cancel batch → `Idle` |
| **Re-run prompt** (6) | activate the **focused** button; **default focus = Skip** (the safe default per §2.5) | cancel the prompt → back to `Destination` (does **not** convert) |
| **Mixed-drop refusal** (9) | dismiss (acknowledge) → `Idle` | dismiss → `Idle` (identical; refusal has no "proceed") |
| **Unsupported / unreadable** (10) | dismiss → `Idle` | dismiss → `Idle` |
| **About dialog** | (no default action) | close |
| **Converting** (7) | — (no Enter action) | request cancel (confirmed round-trip) |

> **Safe-default rule.** Where a gate has a destructive-vs-safe choice, **Enter
> activates the safe option** and it is the pre-focused one: the re-run prompt
> defaults to **Skip**, never to overwriting/duplicating. This encodes the SSOT
> no-harm bias directly into the keyboard model.

### Global shortcuts policy `[DECIDED — none OS-global]`
ConvertIA registers **no OS-global hotkeys** (no system-wide shortcut grabbing) —
all accelerators are **in-app only**, active while the window is focused. This
keeps the portable / no-system-pollution posture (SSOT *Portable*) and avoids the
Tauri global-shortcut permission on the §0.10 allowlist. Menu accelerators (if a
native menu is added) are app-window scoped.

---

## 5.11 Open items (this section's `[OPEN]`s → README open-questions log)

| Item | Why open / lean | Owner |
|------|-----------------|-------|
| **State store library** | Zustand recommended (tiny, selector-granular); genuinely substitutable for any minimal store keeping the IPC-façade + selector rules | §5.1 (this file) — low-stakes, resolvable |
| **Patent-gapped target: disabled-tile-with-note vs omit** | leaning **disabled-with-note** (honest, surfaces *why*); final call depends on §3.4 dispositions actually existing on a platform **and** the §9 usability walkthrough | rendering here, **availability data §3.4** |
| **Theme persistence** | both themes follow OS by default; whether the chosen theme persists is **gated by §7.4** (`[OPEN]`: v1 may persist nothing). If nothing persists, no in-app toggle needed | §7.4 |

> The two **inherited** UI-adjacent opens from 04-formats — the **to-GIF option
> scope** (`[OPEN-E]`, trim in Basic vs Advanced) and the **extract-audio target
> subset** (`[OPEN-A]`) — are **owned by 04-formats/cross-category**, not here;
> this section will render whatever option descriptors §1.6/04 declare
> (OptionsPanel is descriptor-driven), so neither blocks the UI build. Listed here
> only to record the cross-reference, not to claim ownership.
