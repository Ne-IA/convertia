# 01 — Conversion Pipeline (platform-independent core)

> The canonical, engine-agnostic core: how an item goes from "dropped" to
> "converted output", independent of any specific format or OS. (§0.5 is a
> navigational map; **this file owns the pipeline**.)
> Origin: SSOT *How It Feels to Use*, *Recognize files by content*, *Fail clearly*,
> *It just works by default*, *Visible progress, cancellable*.

## Status & decision tags

`[DECIDED]` fixed here / by the SSOT · `[OPEN]` needs an owner-level call (feeds the
README open-questions log) · `[REC]` an `[OPEN]` resolved here with a recommended
default. Parked Phase-1 decisions honoured throughout: **Tauri** (Rust core +
React UI), **bundle everything offline / zero runtime fetch**, **copyleft engines
isolated as separate invoked binaries** (§3.6).

## What this file owns vs references

| Concern | Owner |
|---------|-------|
| The engine-agnostic pipeline stages (1.1–1.12), their order, the in-memory state machine | **this file** |
| Generic option-declaration model (§1.6) | **this file** |
| Generic engine-invocation lifecycle **incl. the cancellation/kill mechanism** (§1.7) | **this file (sole owner)** |
| `OutputPlan` *computation* (§1.8) | **this file** (the *rules* it applies are §2.7; the *write* is §2.1/§2.14) |
| IPC command/event signatures (names, payloads, error shape, channels) | §0.4 (referenced, never restated) |
| Core domain types (`DroppedItem`, `Batch`, `ConversionJob`, `Target`, `OutputPlan`, `RunResult`, `RunId`/`InstanceId`) | §0.6 / §7.1 (referenced) |
| Concurrency degree, worker pool, LibreOffice-serialization rule | §0.9 (referenced) |
| No-harm / atomicity / no-clobber / frozen set / re-run detection / cleanup / error taxonomy / temp & cross-volume strategy / decoder isolation | §2.1–§2.14 (referenced) |
| Per-engine concrete argument construction, progress-signal formats, exit codes | §3.5 (referenced) |
| Engine registry / selection / single-engine-per-pair rule | §3.2 (referenced) |
| Per-format detection signatures, target sets, per-pair options & default *values* | 04-formats (referenced; never restated) |
| UI screen states, async/event subscription lifecycle, virtualization | §5 (referenced) |
| Instance/run identity, startup engine-presence check, OS intake entry points | §7.1 / §7.2 / §7.8 (referenced) |

The pipeline is a **pure orchestrator**: it never parses format bytes itself
beyond detection (§1.2), never constructs engine arguments (§3.5), and never
performs the final write (§2.1). It sequences stages, owns the in-memory job
state, and routes every untrusted-byte operation through the §2.12 isolation
boundary.

---

## 1.0 End-to-end stage flow (the spine)

```
intake (1.1) ─▶ detection (1.2) ─▶ grouping + pre-flight (1.3)
   ─▶ collected-summary + CONFIRM GATE (1.4)
   ─▶ target resolution (1.5) ─▶ options model (1.6)        ── C3 get_targets
   ─▶ destination shown / changeable (OutputPlan preview, 1.8 + §2.7) ── C4 plan_output
   ─▶ [destination change (1.8) re-validates writability/divert] ── C5 set_destination
   ─▶ [re-run detection §2.5 runs IN C4 → OutputPlanPreview.rerun → UI enters
       RerunPrompt (§5.2) BEFORE Convert; the user's RerunDecision rides C6]
   ─▶ resource pre-flight & budgets (1.10, also surfaced in the C4 preview)
   ─▶ CONVERT: per-item engine invocation (1.7) ─▶ OutputPlan finalise (1.8)
        ─▶ atomic write (§2.1/§2.14)  [with live progress 1.11, cancellable 1.7]
   ─▶ end-of-batch summary (1.12)
```

Three **gates** stop automatic progression and require a user action (all UI
states owned by §5.2): the **confirm gate** (1.4, mandatory), the **re-run
prompt** (only if §2.5 fires), and the **mixed-drop refusal** (1.3, a hard
reject, not a gate the user can pass without re-dropping). Everything else flows
to a sensible default with no required choice (SSOT *It just works by default*;
the §1.6 defaulting rule guarantees this).

The whole flow before CONVERT is **cheap and synchronous-feeling** (detection +
grouping + summary build); CONVERT is the only long-running phase and is fully
async (Tokio, §0.9) with real progress (§1.11).

---

## 1.1 Input intake `[DECIDED]`

**Goal:** every way a file can enter ConvertIA arrives as a **real, absolute
filesystem path in Rust**, feeding **one** intake funnel that builds the **frozen
source set** (snapshot; §2.4).

### Entry points (enumerated)

| Entry point | Mechanism | Where paths materialise |
|-------------|-----------|-------------------------|
| **Drag-and-drop** (files or folders) | Tauri **native drag-drop event** (`onDragDropEvent`, payload `type: 'drop'` carries `paths: string[]`) — **not** HTML5 DnD, which does not expose FS paths in a WebView (§0.4) | WebView event → forwarded to a Rust intake command |
| **File picker** | **C2a `pick_for_intake`** (§0.4.1): the native files/folder dialog is opened **Rust-side via `DialogExt`** inside the command handler (no JS `open({…})`, no `dialog:allow-open` grant — §0.10) | Picked paths funnel **straight into this funnel Rust-side**; C2a returns the same `CollectedSet` — no path transits the WebView |
| **Keyboard** | Same **C2a `pick_for_intake`**, invoked via the §5.10 accelerator; full parity (SSOT DoD "keyboard reach the same result") | Same as picker |
| **OS launch entry points** | Open-with / launch args — **macOS** the Tauri v2 **`RunEvent::Opened { urls: Vec<Url> }`** — the **SOLE** macOS file-open mechanism (**NOT** `tauri-plugin-deep-link`'s `on_open_url`, which handles custom-scheme deep links and **never fires** for the Open-With AppleEvent); **Windows** `argv`; **Linux** `%F` desktop-entry field. The macOS payload is **`Vec<Url>` (`file://` URLs), not `Vec<PathBuf>`** — each URL is converted to a path (`Url::to_file_path()` / strip the `file://` scheme) **before** it enters the §1.1 freeze. Each `RunEvent::Opened` (launch AND mid-run Open-With) is routed through the shared `forward_launch_intake` refuse-busy funnel (§7.8.1). Posture (associations: none in v1) owned by §7.8 | Captured at startup / on the macOS `RunEvent::Opened` → url→path → handed to intake |
| **Second-instance hand-off** | When a single-instance policy (§7.1) routes a second launch's args into the running instance | The running instance's intake funnel |

All five funnel into a single Rust function (pseudo-signature; exact IPC names in
§0.4):

```rust
/// One funnel for every entry point. Returns the §0.6 `CollectedSet` (the single
/// discriminated union C1 returns over the wire — §0.4.1): its `Single` variant is
/// the collected batch → 1.4 confirm gate; `Mixed` → 1.3 refusal; `Unsupported` /
/// `Uncertain` → 1.2 decline; `Empty` → "nothing eligible".
fn ingest(paths: Vec<PathBuf>, origin: IntakeOrigin) -> CollectedSet;
// `IntakeOrigin` is §0.6's enum { Drop, Picker, LaunchArg, SecondInstance } — the
// ONE canonical name (this file used "IntakeSource" before; corrected to match
// §0.4/§0.6). There is no separate `IngestOutcome` type: the outcome IS CollectedSet
// (so the §1.2 per-item DetectionOutcome is preserved into the Mixed/Unsupported/
// Uncertain variants — a lone unsupported/uncertain drop yields the specific
// "detected: X" message, not a generic empty report).
//
// Who supplies `origin` per entry point `[DECIDED]`: a DROP / launch-arg / second-
// instance hand-off carries its origin in the C1 request (`Drop` / `LaunchArg` /
// `SecondInstance`). The C2a `pick_for_intake` request has NO `origin` field (the
// WebView only triggers the picker, §0.4.1) — the **C2a handler itself sets
// `origin = IntakeOrigin::Picker`** when it funnels the Rust-opened picked paths into
// this shared `ingest` function. So the WebView never supplies the picker origin; the
// core stamps it. (This closes the "C2a has no origin field but the funnel needs one"
// gap: the funnel always receives a concrete origin — from the request for C1, from the
// handler for C2a.)
```

### Folder recursion (Rust-side) `[DECIDED]`

A dropped/picked **folder** is expanded **recursively in Rust** — the WebView
cannot enumerate a directory (§0.4). Recursion:

- Walks subfolders depth-first (recommended crate: **`walkdir`** for ergonomic
  recursive iteration; `std::fs::read_dir` is the fallback). Symlinked
  directories are **not followed** as a traversal step (loop-safety; the
  resolved-identity de-dup in §2.3 handles file-level link aliasing).
- **Ignores hidden/system files** (SSOT How It Feels 2): names beginning `.`
  (dotfiles) on all platforms, plus the platform sentinels **`.DS_Store`**,
  **`Thumbs.db`**, **`desktop.ini`**, and Windows hidden/system file-attribute
  flagged entries. `[REC]` the ignore list is a fixed constant (not user-config
  in v1); recorded here so §6 can assert it.
- Produces a flat list of candidate file paths; the **dropped root(s)** are
  retained so §2.7 can re-create the relative subtree and "open folder" can open
  the common root.
- **A per-item read/detect failure mid-walk does NOT abort the ingest `[DECIDED]`.**
  Detection runs during the walk (§1.2) and can hit a per-item `Unreadable`/`Empty`/IO
  error (a file that vanished, a denied read, a 0-byte entry). Such an item yields its
  `DetectionOutcome::Unreadable`/`Empty` (→ a §0.6 `SkippedItem` with the matching
  `SkipReason` at the freeze, §1.1 "Empty/Unreadable classification") and the **walk
  CONTINUES** to the next entry — exactly mirroring the mid-run skip rule (§1.9). The walk
  is stopped **only** by an **ingest-scoped C13 `cancel_ingest`** or a **fatal walk-root
  error** (the dropped root itself is unreadable/gone). A single bad file inside a
  thousand-file folder never sinks the whole ingest; it surfaces as a skipped row in the
  confirm summary (§1.4) / §1.12 projection.
- **Cooperatively cancellable** (`[DECIDED]`): the walk + per-item detection loop
  polls an **ingest-scoped `CancellationToken`** keyed by the `CollectingId` (§0.6 —
  **generated by the frontend and passed as a C1 argument**, §0.4.1, so C13 can name the
  in-flight walk before C1 returns), tripped by **C13 `cancel_ingest`** (§0.4.1). On
  cancel it stops the walk and
  **discards the partial, not-yet-frozen set** — there is **no cleanup obligation**
  (no temp/`*.part` is written during ingest; the freeze and any conversion happen
  after). This is what backs the §5.2 *Collecting*-state cancel-collect control,
  needed because a thousands-file recursive walk (§1.10) can run long.
  - **C2a native-dialog phase scope `[DECIDED]`:** for **C2a `pick_for_intake`** the
    Rust-side OS-modal dialog opens **before** any walk begins. **The dialog MUST NOT
    block a Tokio worker thread `[DECIDED]`:** the native picker is opened via `DialogExt`'s
    **async/callback** form (`pick_file`/`pick_folder` with a callback, or spawned on a
    dedicated **blocking** thread via `spawn_blocking`), **never** a synchronous
    `blocking_pick_file` on a Tokio worker — so the async runtime stays free and **C13
    `cancel_ingest` remains serviceable while the modal is up** (a C13 command can run,
    trip the token, and return immediately even though the OS dialog is still on screen).
    To keep C13 honest, the handler **registers the `CollectingId` token at handler entry —
    before opening the dialog** — so a C13 arriving during the dialog **cleanly abandons the
    C2a result** (the handler checks the token after the dialog returns and yields
    `CollectedSet::Empty` rather than walking the picked paths). The OS dialog box itself is
    not force-closed by C13 (no portable API to do so), but its result is discarded — so C13
    is never a silent no-op. **Token drop on EVERY C2a exit branch `[DECIDED]`:** the
    `CollectingId` ingest-token is **dropped/de-registered in every C2a return path** —
    the cancelled-dialog→`CollectedSet::Empty` branch, the C13-tripped→`Empty` branch, **and**
    the normal walk-completes branch — mirroring the **C1 drop-on-return rule** (§0.4.4):
    the §1.1 walk loop that normally drops the token does not run on a cancelled dialog, so
    the handler MUST drop it explicitly there too, or the token leaks in the registry. (A
    drop/launch-arg C1 has no dialog phase; the token covers the whole walk and is dropped
    on the C1 return.)

### Freeze point `[DECIDED]`

Intake is the **exhaustive freeze point** (§2.4): the moment `ingest` snapshots
the set, that set is closed. Files appearing afterward — written by this run, a
concurrent instance, or anything else — are **never** ingested into this run, and
outputs landing in a watched source folder do **not** expand or restart the
batch. The freeze covers the launch-time and second-instance hand-off explicitly,
and its behaviour is **gated by whether a run is in flight `[DECIDED]`** (consistent
with the §7.1.1/§7.8.1 refuse-busy decision):

- **While IDLE** (no run in flight — the app is in `Idle`/`Summary`, §5.2): a macOS
  `RunEvent::Opened { urls }` / Windows-argv / Linux-`%F` / second-instance hand-off
  **starts a NEW frozen set** — after its `file://` URLs are converted to paths
  (macOS) — exactly like a fresh drop, never mutating a frozen one.
- **While a RUN IS IN FLIGHT** (mid-`Converting`): the launch-intake is
  **refused-busy** per §7.1.1/§7.8.1 — the shared `forward_launch_intake` funnel both
  launch hooks call performs the busy check **before** the freeze, so the paths are
  **dropped** (no new set, no merge, no replace) and the `BusyNotice` surface (§5.3) is
  shown. It is **never** ingested mid-run, on any platform (the earlier "starts a new
  batch mid-run" reading is corrected — a mid-conversion Open-with is refused, not
  merged).

The "never mutating a frozen one" invariant holds in **both** cases (an in-flight run's
frozen set is untouched; an idle launch starts its own fresh set; interaction with §7.1
instance policy). De-duplication of the frozen
set **by resolved identity** is owned by §2.3 and applied here as the set is
built (a file reached via two paths is one member).

**Zero-byte / unreadable at intake `[DECIDED]`:** detection runs **pre-flight**
(§1.2), so a 0-byte or already-unreadable file is classified **at intake** and is
recorded as **Skipped**, not silently dropped — the user dropped it, so the summary
must account for it. Concretely: an intake-time empty/unreadable item becomes a
§1.3 `SkippedItem` with `JobState::Skipped(SkipReason::Empty | SkipReason::Unreadable)`
(§0.6/§1.9) — it **never enters the queue** and is surfaced in the §1.4 confirm
summary and the §1.12 totals' `skipped` count (NOT the `failed` count). The
**turn-time** case is distinct: a file that was readable at freeze but becomes
unreadable/gone **when its turn comes** mid-run is a per-item **`Failed`**
(`Unreadable`/`Gone`, §2.8) and counts as a failure (§1.9 mid-run skip). So:
**intake-time empty/unreadable = Skipped (pre-flight); turn-time unreadable/gone =
Failed (mid-run)** — these are different totals and must not be conflated.

---

## 1.2 Content-based format detection `[DECIDED]`

Detection answers, per item: **what is this, really?** — never trusting the
extension (SSOT *Recognize files by content*). It drives **both** eligibility and
batch grouping (§1.3).

### Strategy (layered)

1. **Magic-byte / signature sniff** on a bounded header window (recommended read:
   **first 4 KiB**, plus a small trailer probe for the formats that need it, e.g.
   JPEG `FF D9`). The concrete signatures live in **04-formats** per format
   (e.g. PNG `89 50 4E 47…`, `%PDF-`, RIFF/`ftyp` boxes, EBML `1A 45 DF A3`,
   ASF GUID) — **not restated here**.
2. **Container introspection** where the magic is generic and shared:
   - **ZIP-family disambiguation** (`50 4B 03 04`): read the archive's
     `[Content_Types].xml` / ODF `mimetype` member to tell DOCX vs XLSX vs PPTX
     vs ODT/ODS/ODP (rule owned per file in 04; detection performs the peek).
   - **OLE2 / CFB** (`D0 CF 11 E0…`): inspect the stream directory to tell legacy
     DOC vs XLS vs PPT.
   - **`ftyp` box brand**: MP4 vs MOV vs M4V vs 3GP vs AVIF/HEIC vs M4A.
   - **Codec-inside-container probe**: an `.m4a` is AAC vs ALAC; an Ogg page is
     Vorbis vs Opus; a video container's inner codecs (used later by §3.5's
     remux-vs-re-encode decision, but the *user-facing source type* is the
     **container**, e.g. MKV). The probe depth here is **header-level only**; the
     full `ffprobe` stream inventory is an engine-layer concern (§3.5), invoked
     later, not during the cheap detection pass.
   - **gzip wrapper (`.svgz`)** `[DECIDED — pure-Rust bounded inflate, stays in-core per §2.12.4]`:
     a file whose magic is **`1F 8B`** (gzip) is not itself a recognised format
     — ConvertIA **inflates one bounded block** and re-sniffs the inner bytes; if the
     inner content is an SVG root (`<svg` after optional `<?xml`/BOM/DOCTYPE), the file is
     classified **`Svg`** (the `.svgz` compressed-SVG case the corpus §6.4.5 requires — it
     does **not** decode as text, so it must be caught here, not in step 3, or it would
     drop silently as unrecognised). **Decoder choice (so the §2.12.4 isolation absolute
     is not violated):** the inflate is done with a **pure-Rust DEFLATE** — `flate2` pinned
     to the **`rust_backend` feature (miniz_oxide, safe Rust, no C compiler)**, NOT a
     zlib/zlib-ng C backend — so no third-party **C/C++** decoder runs inside the Rust core
     on untrusted bytes. It is **strictly bounded**: read at most **§-pinned MAX_SVGZ_SNIFF
     = 64 KiB** of inflated output and enforce a **decompression-ratio cap (≤ 100×)**,
     aborting (→ `UnsupportedType`) on either limit — defeats the decompression-bomb class.
     This sniff stays in-core per the §2.12.4 `[DECIDED]` (resolved in the consolidation
     pass): the pure-Rust bounded inflate, the text-encoding heuristic, and the Rust ZIP
     central-directory peek all stay outside the §2.12 isolation boundary (memory-safe,
     bounded, no third-party C/C++ decoder — see §2.12.4 / the README resolved log). Cross-ref images.md (SVG `.svgz` handling — the worker re-inflates with
     librsvg's own bounded loader for the actual raster). Other gzip-wrapped content is
     `UnsupportedType` ("detected: gzip archive").
3. **Text classification** for the magic-less formats (TXT/MD/CSV/TSV/SVG): confirm
   the bytes decode as text (BOM → strict UTF-8 → single-byte codepage fallback),
   then apply the per-file rules (SVG root element; CSV/TSV delimiter sniff;
   TXT-vs-MD by extension/intent). Encoding/delimiter specifics owned by the 04
   files. (Note: `.svgz` is gzip, **not** text — caught by the gzip rule in step 2,
   not here.)
4. **Bounded structural-peek for the §1.4 summary `notes`** (cheap, still
   header/member-level — the producer of `CollectedSummary.notes`): once a type is
   recognised, ConvertIA may read a **small, bounded** structural fact needed for the
   confirm-gate summary line, **without** a full decode:
   - **`>1 sheet`** (spreadsheets) — a bounded ZIP-member read of `xl/workbook.xml`
     (XLSX) / the ODS `content.xml` sheet count / OLE2 directory (XLS); cross-ref
     spreadsheets.md (its multi-sheet `[DECIDED]` — picker defaulting to active sheet).
     Drives the "only one sheet is exported" note (§2.9 `sheet_to_delimited`).
   - **`animated source present`** (images) — a bounded descriptor-count peek: GIF
     image-descriptor count, WEBP `VP8X` animation flag / `ANMF` chunks, APNG `acTL`
     chunk, AVIF `avis` brand; cross-ref images.md animation policy. Drives the
     "animated — only the first frame is converted" note (§2.9
     `image_animation_flatten`) at the summary level.
   - **`>1 icon size`** (ICO source) — a bounded read of the **`ICONDIR`** header's
     **entry count** (the 6-byte header's `idCount` field + the fixed-size `ICONDIRENTRY`
     table); count `> 1` ⇒ a **`MultiSizeIcon`** note (its `detail` carries the size list,
     e.g. "16, 32, 48") — images.md ICO source holds multiple sizes. Header-level only, no
     image decode.
   - **`embedded cover art present`** (audio) — a bounded tag-peek for an attached
     picture: **ID3v2 `APIC`** frame (MP3), **FLAC `PICTURE`** metadata block, **MP4
     `covr`** atom (M4A/AAC); presence ⇒ **`EmbeddedCoverArt`** note. Bounded tag/metadata
     read only (no audio decode); cross-ref audio.md cover-art handling.
   - **raster pixel dimensions** (raster image sources) — a bounded **header** read of the
     intrinsic width/height: **JPEG `SOF` marker**, **PNG `IHDR`**, **GIF logical-screen
     descriptor**, **BMP `BITMAPINFOHEADER`**, **TIFF `ImageWidth`/`ImageLength` IFD tags**,
     **WEBP `VP8`/`VP8L`/`VP8X` header**. This populates **`DetectionOutcome::Recognized.dims:
     Option<(u32,u32)>`** (`None` for non-raster or unreadable header) — the load-bearing
     input to the §1.10 per-pixel size estimate (which consumes `dims`, never decodes).
     Header-level only, no image decode; bounded in-memory read in memory-safe Rust. (This
     peek produces `Recognized.dims`, not a `CollectedNoteKind` note — the four note
     producers above plus this dims producer together are the complete step-4 output.)
   These peeks are **bounded member reads in memory-safe Rust** (no third-party
   decoder, §2.12), so they stay in-core and cheap; they run only for the relevant
   detected types, not every item. **`CollectedSummary.notes` (§1.4) is produced
   here** — **all four typed `CollectedNoteKind` variants** (`MultipleSheets`,
   `AnimatedSource`, `MultiSizeIcon`, `EmbeddedCoverArt`) have a declared producer in this
   step. The fifth variant, **`Other`**, is a **reserved forward-compatible extension point
   emitted by no current (v1) engine** — it carries an arbitrary `detail` so a future
   detection note can be surfaced without a wire-type change, and is rendered via the §5
   generic-note fallback if ever produced. So every *typed* variant has a producer and the
   one catch-all is intentionally unproduced-in-v1 (not an unreachable bug).

### Detection result model `[DECIDED]`

`DetectionOutcome` below is the **single canonical detection type** — §1.2 owns it and
§0.6 references it (it is the type carried by `DroppedItem.detected`). There is **no
separate `DetectedFormat`/`DetectionConfidence` pair** (an earlier draft defined a
3-valued confidence enum and an `Option<UserFacingFormat>` that collapsed
Empty-vs-Unreadable; both are retired). `Confidence { High, Low }` here is the one
confidence enum (one name, two values) across both files.

```rust
struct DetectionResult {
    item: ItemId,
    outcome: DetectionOutcome,
}

enum DetectionOutcome {
    /// A supported v1 source type, with confidence. `dims` carries the
    /// **header-derived pixel width/height** for raster formats (JPEG SOF, PNG IHDR,
    /// etc.), read by the bounded structural-peek (step 4) — `None` for non-raster or
    /// where the header lacks them. It is the input the §1.10 cheap per-pixel size
    /// estimate consumes, so the estimate never needs a decode (§1.10 "where the cheap
    /// estimate's inputs come from"). Mirrored on the wire (§0.4.5).
    Recognized { format: UserFacingFormat, confidence: Confidence, dims: Option<(u32, u32)> },
    /// A real type we identified but do not convert (SSOT: "can't convert this
    /// type — detected: X"). Carries the named type for the message.
    UnsupportedType { detected: String },
    /// Decoded/sniffed but the signal is contradictory or below threshold.
    /// (SSOT: name what we think it is, or that we can't tell, decline clearly.)
    Uncertain { best_guess: Option<String> },
    /// 0-byte / no bytes to read.
    Empty,
    /// Could not read the bytes at all (gone/locked/permission).
    Unreadable { reason: ReadFailure },
}

enum Confidence { High, Low }   // Low never silently falls back to the extension

/// Why a file's bytes could not be read at freeze/detect time (§1.2). Owned here;
/// the §2.8 taxonomy projects these to a plain-language string. Distinct from a
/// conversion-time failure (that is §2.8 `ConversionErrorKind`).
enum ReadFailure {
    NotFound,        // gone between drop and freeze (§2.4)
    PermissionDenied,// OS denied read
    Locked,          // exclusively locked by another process (esp. Windows)
    IoError,         // any other OS read error
}
```

**Outcome rules (SSOT-bound):**
- `UnsupportedType` and `Uncertain` and `Empty`/`Unreadable` are **never** offered
  a target list and **never** silently extension-fallback or guessed
  (SSOT *Recognize files by content*). They are surfaced (eligible=false) with the
  exact §2.8 plain-language string.
- A file whose extension lies (a `.jpg` that is really PNG) is grouped and
  converted as its **detected** type.
- Detection feeds §1.3 grouping by the **individual user-facing format** the
  `UserFacingFormat` maps to (not the six categories, not codec subtypes).

### Where detection runs — security `[REC]`

Detection is **the first code that touches untrusted bytes**, so its placement
relative to the §2.12 isolation boundary is security-relevant:

- **Header-only magic sniff + ZIP/OLE/`ftyp` structural peeks + text/encoding
  classification + the `.svgz` bounded inflate run in-core** (in the Rust process).
  `[REC]` rationale: these are bounded reads parsed by **memory-safe Rust** crates (e.g.
  `infer`/custom matcher for magic, a Rust ZIP reader for the content-type member, an
  encoding detector such as `chardetng`, and **`flate2` pinned to its `rust_backend`
  (miniz_oxide) feature** for the `.svgz` 1F-8B inflate — pure safe Rust, no C decoder)
  over a **capped window** (the `.svgz` inflate additionally capped at ≤64 KiB + ≤100×
  ratio) — they do not invoke a third-party C/C++ decoder, so the classic decoder attack
  surface (§0.11 "untrusted decoder input") is not yet engaged. Keeping detection in-core
  makes the cheap pass fast (no subprocess per item for thousands of files).
- **The full decode** (anything that hands bytes to libvips/FFmpeg/LibreOffice/
  poppler/pandoc) happens only at **conversion time**, **inside** the §2.12
  boundary (§1.7 routes every invocation through the isolation wrapper).
- `[DECIDED — owner §2.12.4]` the **text-encoding heuristic**, the Rust-side ZIP
  central-directory parse, and the **`.svgz` pure-Rust bounded inflate** **stay in-core**
  (outside the §2.12 isolation boundary): all three are memory-safe, bounded (the `.svgz`
  inflate capped ≤64 KiB + ≤100× ratio), none is a full decode, and **none links a
  third-party C/C++ decoder**, so none violates the §2.12.4 "no third-party C/C++ decoder
  in-core" absolute (worded exactly that way for this reason). §2.12.4 owns the final
  isolation-boundary line and confirms this disposition; no isolation subprocess is spun
  up for a detection sniff.

---

## 1.3 Batch grouping & the pre-flight rule `[DECIDED]`

### Grouping key

The grouping key is the **individual user-facing format** (SSOT How It Feels 3,
Principle 6) — **not** the six scope categories and **not** codec-level subtypes:

- `.jpg` ≠ `.png`; **MP4 ≠ MOV ≠ MKV** (container, not codec); **MP3 ≠ WAV**;
  **OGG(Vorbis) ≠ OPUS** (codec ID, distinct user-facing formats); **M4A(AAC) ≠
  ALAC**; **CSV ≠ TSV** (delimiter-determined). These distinctions are exactly the
  detection outputs of §1.2, settled per format in 04.
- A **multi-category** format (e.g. PDF, shared by documents/sheets/slides) is
  **one** detected type and is offered the **de-duplicated union** of its sensible
  targets (rule owned by §1.5; assembled in the 04 canonical home).

`UserFacingFormat` (the §0.6 enum) **is** the grouping key. Two items group together
iff their `UserFacingFormat` is equal.

### v1 batch rule: one source format at a time `[DECIDED]`

```rust
// `Grouping` is an INTERNAL projection that maps onto §0.6's wire/domain
// `CollectedSet` (the type C1 returns). The mapping preserves the §1.2 per-item
// `DetectionOutcome` so a single Unsupported/Uncertain drop produces the specific
// "detected: X" message (not a generic empty report):
//   Single → CollectedSet::Single ; Mixed → CollectedSet::Mixed{found} ;
//   a lone Unsupported → CollectedSet::Unsupported{detected} ;
//   a lone Uncertain → CollectedSet::Uncertain{note} ;
//   otherwise → CollectedSet::Empty{skipped} (skip reasons projected from EmptyReport.outcomes).
fn group(detected: Vec<DetectionResult>) -> Grouping;

enum Grouping {
    /// Exactly one eligible source format across all readable items.
    /// `SkippedItem` is the §0.6 type (owner): { item, source, reason: SkipReason }.
    /// `members` (eligible ItemIds) and `skipped`'s ItemIds are id-DISJOINT views over
    /// the SINGLE id space assigned at the freeze over ALL dropped items (§0.6 invariant
    /// 6) — never re-indexed from 0, so the two never collide.
    Single { format: UserFacingFormat, members: Vec<ItemId>, skipped: Vec<SkippedItem> },
    /// Two or more distinct eligible source formats → hard pre-flight refusal.
    Mixed(MixedReport),
    /// No eligible source at all — carries the per-item DetectionOutcomes so a lone
    /// unsupported/uncertain drop maps to the specific CollectedSet variant above.
    Empty(EmptyReport),
}

struct MixedReport { found: Vec<(UserFacingFormat, usize)> } // e.g. [(JPG,30),(PNG,12)]

/// Carries the per-item detection outcomes of an all-ineligible drop so `group()` can
/// project the SPECIFIC CollectedSet variant (not a generic empty), per the mapping above.
struct EmptyReport { outcomes: Vec<DetectionResult> }         // every item's §1.2 outcome
```

**`Empty(EmptyReport)` → CollectedSet projection rule `[DECIDED]`.** When `group()`
finds **no eligible source**, it inspects `EmptyReport.outcomes` and projects, in this
order: **(1)** if there is **exactly one** item and its outcome is
`DetectionOutcome::UnsupportedType { detected }`, → `CollectedSet::Unsupported { detected }`
(the detected-but-unsupported format, so the §5.2 state-10 copy can name it); **(2)** if
there is **exactly one** item and its outcome is `DetectionOutcome::Uncertain { best_guess }`,
→ `CollectedSet::Uncertain { note }` (the §1.2 uncertainty note, from `best_guess`); **(3)**
otherwise (zero items, or 2+ ineligible items of mixed/none kinds) →
`CollectedSet::Empty { skipped }` (the generic "nothing here I can convert") — **`skipped`
is projected from `EmptyReport.outcomes`**: each ineligible item becomes a `SkippedItem
{ item, source, reason: SkipReason }` (§0.6), so the per-item skip reasons §5.2 state-10
shows are **carried on the wire**, not discarded (a 2+ all-ineligible drop no longer
collapses to a reason-less Empty). The genuinely-zero-items case (cancelled dialog /
drained-empty `PendingIntake`) is `Empty { skipped: vec![] }`. This is the single
owner of the lone-Unsupported / lone-Uncertain specificity; §5.2 row 2 routes all three
to the *Unsupported* screen (state 10) with the variant-specific copy.

- **`Single`** → proceeds to the confirm gate (1.4). `skipped` carries the
  per-item ineligibles (unsupported/uncertain/empty/unreadable) so the summary and
  the collected-set display can show "N collected, M skipped (why)".
- **`Mixed`** → **hard pre-flight refusal** (SSOT How It Feels 3): ConvertIA does
  **not** convert a subset. It names the formats it found **with counts** and asks
  the user to **re-drop a single format**. There is **no** "convert just the JPGs"
  affordance in v1 (mixed-format handling is parked — SSOT *Future Ideas*). This is
  a **distinct** behaviour from skipping one bad item mid-run (§1.9): the mixed
  refusal happens **before** any conversion and rejects the whole drop.
- **`Empty { skipped }`** → "nothing here I can convert" with the detected reasons
  (e.g. "all files were unreadable" / "all of an out-of-scope type") — the §0.6
  `skipped: Vec<SkippedItem>` payload carries the per-item reasons so §5.2 state-10 can
  tally them ("N files, none convertible — M unreadable, K unsupported, …", using the
  §0.6 `SkipReason` set); the reasons are no longer lost when 2+ ineligible items collapse
  to Empty. The **all-hidden** drop is the genuinely-zero-items case (hidden/system files
  are walk-filtered and never become `SkippedItem`s, §1.1) → `Empty { skipped: vec![] }`,
  rendered with the plain "only hidden files were found" copy (no tally).

**De-dup interaction:** the resolved-identity de-dup (§2.3) runs in §1.1 as the
set is frozen, so by grouping time each member is a unique resolved file. Two
dropped paths pointing at one file are one member of one group.

---

## 1.4 Collected-set summary & confirm gate `[DECIDED]`

### Payload (owned here; UI state in §5.2)

After a `Single` grouping, the pipeline produces the **collected-summary payload**
— the backend data the confirm screen renders:

**Wiring `[DECIDED]`.** `CollectedSummary` is **not a separate wire type** — its field
set **is** the §0.6 `CollectedSet::Single` payload (the two were unified in the
convergence pass so the mandatory confirm gate has a real IPC path: C1/C2a already
return `CollectedSet`, and its `Single` variant now carries `total_bytes`, `roots`,
`encoding_hint`, `delimiter_hint`, `notes` alongside `id`/`format`/`count`/`items`/
`skipped`). `CollectedSummary` below is therefore the **display/projection name** for
exactly those `CollectedSet::Single` fields the confirm screen renders — `§0.6 owns the
struct shape, §1.4 owns the confirm-gate semantics`. No extra `get_collected_summary`
command exists; the confirm screen renders the `CollectedSet::Single` C1/C2a returned
(re-fetchable from the §0.4.4 collected-set registry by `collectedSetId` if the WebView
reloads):

```rust
// Projection of §0.6 `CollectedSet::Single` (NOT a redefinition). These ARE the
// Single-variant fields (§0.6 is the owner); listed here so §1.4 reads standalone.
struct CollectedSummary {            // == the §0.6 CollectedSet::Single field set
    collected_set_id: CollectedSetId, // == the §0.6 CollectedSet::Single.id
    format: UserFacingFormat,    // detected, user-facing (e.g. "JPG") — §0.6 enum
    count: usize,                // e.g. 48  → "48 JPG files"
    total_bytes: u64,            // for the size hint / 1.10 pre-flight
    roots: Vec<PathBuf>,         // dropped root(s) → relative-subtree + open-folder
    skipped: Vec<SkippedItem>,   // ineligibles, §0.6 type { item, source, reason: SkipReason }
    // detection-derived hints surfaced in the summary line (per 04):
    encoding_hint: Option<String>,   // e.g. CSV detected "Windows-1252"
    delimiter_hint: Option<String>,  // e.g. CSV/TSV detected ";"
    notes: Vec<CollectedNote>,   // e.g. ">1 sheet", "animated source present" —
                                 // PRODUCED by §1.2's bounded structural-peek (step 4),
                                 // not invented here (spreadsheets.md / images.md own
                                 // the per-format peek; §1.2 owns running it)
}

/// A detection-derived informational note surfaced in the confirm summary (§1.4).
/// Owned here (§1.4). A stable `kind` (so §5 can localise via §2.10) plus an optional
/// detail value; never a pre-localised sentence. The four `kind` discriminants are
/// MultipleSheets, AnimatedSource, MultiSizeIcon, EmbeddedCoverArt — each a BARE variant
/// (no inline payload); any value (sheet count, icon size list, …) rides the `detail:
/// Option<String>` field below, NOT the enum variant.
struct CollectedNote {
    kind: CollectedNoteKind,     // stable discriminant → §5 label catalogue (§2.10)
    detail: Option<String>,      // optional value (e.g. "3 sheets", "Windows-1252")
}

enum CollectedNoteKind {
    MultipleSheets,              // spreadsheets.md: >1 sheet, only one exported
    AnimatedSource,              // images.md: animated source → still target flattens
    MultiSizeIcon,               // images.md: ICO source holds >1 size
    EmbeddedCoverArt,            // audio.md: cover art present
    Other,                       // catch-all carrying `detail`; never silently dropped
}
```

### The confirm gate `[DECIDED]`

A **mandatory** pre-convert gate (SSOT How It Feels 3): **before converting,
ConvertIA shows what it collected** (format + count, e.g. "48 JPG files"),
*especially* for recursively collected folders where the user cannot see the file
count any other way. The user confirms (or cancels / re-drops). The gate is
satisfied by an explicit affirmative action (button / Enter, per §5.10). It is the
**only always-present interstitial** between drop and target choice.

The summary line is *informational only* — it does not require choices; choices
(target/options) come after, on the next screen (§1.5/§1.6). Combining the
confirm and target screens into one view is a §5 layout decision; the **gate
semantics** (an explicit confirm exists, batch is shown first) are fixed here.

---

## 1.5 Target resolution `[DECIDED]`

### From source → offered targets

Given the single `UserFacingFormat`, the pipeline resolves the **offered target set**
from the **engine/format registry** (§3.2 owns the registry; the concrete
per-source target lists and the single default live in **04-formats** and are
**not restated here**). The §0.4.1 **C3 `get_targets`** command returns this set:

```rust
fn resolve_targets(src: UserFacingFormat, platform: Platform) -> TargetOffer;
// Returns the §0.6 `TargetOffer` (the C3 return type). §0.6 OWNS the struct
// (`TargetOffer { set, targets: Vec<Target>, default_target }` and `Target`);
// §1.5 describes only the resolution LOGIC (mirroring the §1.6
// `EffectiveOptions == OptionValues` reconciliation). The earlier
// `OfferedTargets`/`OfferedTarget` names are retired in favour of §0.6's
// `TargetOffer`/`Target`. For reference, §0.6's `Target` carries:
//   id: TargetId        // §0.6 TargetId = Format(FormatId) | Op(CrossCatOp)
//   availability        // §0.6: Available | Unavailable { reason } per §3.4
//   lossy: Option<LossyKind>  // predictable-loss marker → §2.9 (the ONE canonical
//                             //   name: §2.9's LossyKind). String lives in §2.9.
// The offered target's kind is carried by `TargetId` itself (no separate TargetKind).
```

**Rules this section owns (general; per-source specifics in 04):**

1. **One detected type → de-duplicated union of its sensible targets.** A
   multi-category format (PDF) yields the union of document/sheet/slide-side
   targets, de-duplicated (assembled in the 04 canonical home, e.g. PDF in
   `documents.md`). The pipeline does not re-derive the matrix; it reads the
   registry.
2. **Cross-category outputs are *targets of the source*, not a second source**
   (SSOT *Direction & shape rule*). A video source's offered set is its video
   targets **plus** `extract-audio` **plus** `to-GIF` (the closed set, owned by
   `cross-category.md`). Choosing one applies to the **whole same-source batch**.
3. **Exactly one pre-highlighted default per source** (SSOT How It Feels 4). The
   **general defaulting rule** is owned here; the **per-source default value** is
   marked in each 04 matrix:
   > *Tie-break favours a widely-compatible target, unless a modern format
   > (WEBP/AVIF/OPUS) is clearly the better everyday choice; AVIF/HEIC are never a
   > default target. Same-format (diagonal) is never offered as a target tile.*
   The pipeline trusts the registry's `default` flag (sourced from 04); it does
   not invent it.
4. **One `Target` per `Batch`** (the §0.6 invariant): **per-file target selection
   is out of v1.** The chosen `TargetId` is a batch-level property.
5. **Same-format diagonal** is excluded from the offered tiles (per 04; e.g. images
   omit same→same). **The ONLY v1 diagonal exception is the video "normalize" self-target
   `[DECIDED]`** — **owned by video.md** (the MP4→MP4 / MOV→MOV / MKV→MKV / WEBM→WEBM / M4V→M4V
   normalize/`+faststart` self-target, video.md §"Same-container"); MP3→MP3 and all other
   audio/image/office same-format diagonals are **NOT v1** (README `[DECIDED]`). The registry
   encodes which diagonals are offered, and only the video normalize diagonal is enabled in v1.

### Patent-gapped / unavailable targets `[DECIDED — routing only]`

A target may be `Unavailable` on the current platform per the **§3.4 patent
disposition matrix** (HEIC/AAC/H.264 × platform). The pipeline **reads** §3.4's
verdict via the registry and marks the `Target.availability` (§0.6); it never
re-decides it. Whether an unavailable target is **omitted vs shown-disabled-with-
note** is a §5.2 presentation decision sourced from §3.4. The **default** is
guaranteed `Available` on every shipping platform: if a per-source default would
be gapped, that is a §3.4/category product problem (notably MP4-as-default video
depends on H.264/AAC shipping everywhere — flagged by video.md and §3.4), not a
silent omission here.

---

## 1.6 Options model — **owner of the generic option-declaration model** `[DECIDED]`

This section owns the **generic** model only. **Concrete per-pair option lists and
default *values* live in 04** (per-source) and are **not restated** here.

### Generic option declaration

```rust
/// A declared option for the (source, target) pair, supplied by the registry
/// (values defined in 04). The pipeline renders/collects these generically.
struct OptionDecl {
    key: OptionKey,
    label: LabelKey,             // UI-chrome string → §5 (not §2.8/§2.9)
    surface: Surface,            // Basic | Advanced
    kind: OptionKind,
    default: OptionValue,        // the no-decision default (from 04)
}

enum Surface { Basic, Advanced }

enum OptionKind {
    /// Bounded integer (quality/CRF/compression level) with a range + optional unit.
    IntRange { min: i64, max: i64, step: i64, unit: Option<Unit> },
    /// A small named preset set (e.g. MP3 High/Standard/Small) mapping to engine flags.
    Enum { choices: Vec<EnumChoice> },
    /// A boolean toggle (lossless on/off, progressive, BOM).
    Toggle,
    /// A pixel/size value (SVG width, GIF width).
    Size { min: u32, max: u32 },
    /// A colour (flatten background) — picker; default usually white.
    Color,
}

/// The effective, resolved option set for a batch — feeds §1.7 (engine args via
/// §3.5), §2.5 (re-run equivalence keys on these), and §1.10 (size estimate).
/// This IS §0.6's `OptionValues` (the wire/domain name); `EffectiveOptions` is the
/// in-pipeline alias for the same `BTreeMap<OptionKey, OptionValue>`. There is no
/// raw-vs-resolved distinction in v1: the registry's declared `OptionDecl.default`s
/// merged with user overrides ARE the resolved values.
struct EffectiveOptions(BTreeMap<OptionKey, OptionValue>); // == §0.6 OptionValues

// ─── The option-model leaf types (defined here, §1.6 is their owner) ─────────
/// Stable machine key for an option (e.g. "quality", "fps", "lossless"). Used as
/// the BTreeMap key and in the §2.5 EquivKey canonicalisation, so it must be a
/// stable string, never a UI label. Newtype over a short ASCII slug.
struct OptionKey(String);

/// A UI-chrome label key (§5 resolves it to a localised string, §2.10). NOT a
/// user-facing string itself — keeps the domain model i18n-free (§2.8/§2.9 own
/// surfaced strings; this is §5's catalogue key).
struct LabelKey(String);

/// One concrete, fully-resolved option value. INVARIANT: every variant is
/// JSON-serialisable (it crosses the §0.4.5 tauri-specta wire) and round-trips
/// through the §2.5 canonical form. No floats with NaN/Inf; colours as #RRGGBB(AA).
enum OptionValue {
    Int(i64),               // IntRange / Size resolved value
    Bool(bool),             // Toggle
    Enum(String),           // the chosen EnumChoice.value (stable id, not the label)
    Color(String),          // "#RRGGBB" or "#RRGGBBAA"
}

/// A named preset choice inside an Enum option (e.g. MP3 "High"/"Standard"/"Small").
struct EnumChoice {
    value: String,          // stable id stored in OptionValue::Enum (never localised)
    label: LabelKey,        // §5 UI-chrome label for the choice
}

/// Display unit for an IntRange (purely for the §5 label; not semantic).
enum Unit { Percent, Kbps, Px, Dpi, Fps }
```

### Basic vs Advanced `[DECIDED]`

- **Basic**: the few switches that *materially* change a normal user's result
  (e.g. WEBP quality, GIF fps/width, the re-encode "Quality" slider). Shown
  directly.
- **Advanced**: many/niche options, tucked behind an **"Advanced options"** drawer
  so the default view stays clean (SSOT How It Feels 5; §5.3 owns the drawer
  component).
- **v1 exposes only settings that materially change a normal user's result** —
  adding a setting is a **scope change**, not a default. Rich per-format option
  sets and remembered presets are **out of v1** (SSOT). Some categories expose
  **none** (documents/PDF, ALAC, BMP).

### The no-decision defaulting rule `[DECIDED]`

**Every option has a default; the common path requires zero choices** (SSOT
*It just works by default*, Principle 8). The pipeline builds `EffectiveOptions`
by taking each `OptionDecl.default` unless the user overrode it. Converting with
zero interaction (drop → confirm → default target → convert) is always valid.

**The one structural exception:** where a target is *degenerate without a required
choice* (same-format re-encode, parked per 04 — e.g. MP3→MP3 needs a target
bitrate to be non-degenerate), the pair is **not offered** in v1 rather than
introducing a required choice. So the no-required-choices rule holds without
exception for every *offered* pair.

### Defaults registry & the DoD gate `[DECIDED]`

`[DECIDED]` **§1.6 owns a CI-generated consolidated defaults registry** (the merged
index of every `OptionDecl.default` across all `(source,target)` pairs, sourced from
04). This is the **single machine-checkable home** of the SSOT *v1 DoD* "no required
choices" gate: the drop→default→convert, zero-clicks promise needs exactly one place
asserting "every offered `(source,target)` pair has a **complete** default option set."
**The registry is a CI-generated artifact `[DECIDED]`:** §6.7.1 Lane-A generates the
merged `OptionDecl.default` index from the 04 tables and runs a **guard that FAILS the
build if any §04-offered pair lacks a default** for any of its declared options (so a
new pair or option that ships without a default cannot pass Lane-A). The **values**
remain owned by 04; this is the assembled/verified index, not a second source of
truth. (Escalated from `[REC]`: this gate had no other committed home, so §1.6 commits
to owning it; §6.10 DoD row 7 reads "owned by §1.6", not "may own".)

---

## 1.7 Engine-invocation model — **generic owner (incl. cancellation/kill)** `[DECIDED]`

The engine-agnostic subprocess lifecycle. **Per-engine concrete argument
construction, progress-signal parsing, and exit-code/`stderr` quirks live in
§3.5** (not restated here). Every invocation runs **through the §2.12 isolation
wrapper**. This section is the **sole owner of the cancellation/kill mechanism**;
§0.9, §1.10, §1.11, §5.8 and the 04 files reference it.

### Invocation lifecycle (state machine per item)

```
spawn ─▶ Running ──(progress events)──▶ ...
            │
            ├──▶ exit 0  ─▶ verify output ─▶ Succeeded
            ├──▶ exit ≠0 / stderr-classified ─▶ Failed(kind)   [→ §2.8]
            ├──▶ timeout / no-progress ─▶ kill ─▶ Failed(EngineHang) [→ §2.8]
            ├──▶ user cancel ─▶ kill ─▶ Cancelled
            └──▶ spawn error (binary missing/denied) ─▶ Failed/AppFault [→ §2.13]
```

**`EngineInvocation` is the dispatch envelope, NOT a second plan type.** `[DECIDED]`
The plan-time artifact is the §3.2.2 **`Invocation`** returned by `Engine::plan()` (it
owns `program`/`args`/`cwd`/`env`/`stdin`/`progress`/`out_tmp` — the single source of
the argv/cwd/env). `EngineInvocation` (this section) is only the **dispatch envelope**
the §1.7 lifecycle submits to the §0.9 pool: it wraps `(JobId, EngineId, Invocation,
CancellationToken)` and adds nothing the §3.2 `Invocation` already carries. It does
**not** re-declare argv/work_dir/env (those live in the wrapped `Invocation`):

```rust
struct EngineInvocation {
    job: JobId,
    engine: EngineId,            // (see §0.6 EngineId — the canonical variant set)
    plan: Invocation,            // §3.2.2 — the plan artifact: program/args/cwd/env/stdin/out_tmp
    cancel: CancellationToken,   // tokio_util::sync::CancellationToken
}

enum InvocationResult {
    Succeeded,
    Failed(ConversionErrorKind), // §2.8 taxonomy (the Rust-internal kind, §2.8 owner);
                                 //   the orchestrator (crate::run) maps it to the wire `ErrorKind`
                                 //   via `ErrorKind::from(kind)` at the §1.9 Running→Failed transition
                                 //   (From impl owned by crate::outcome) and again at the §0.4.3 IPC
                                 //   boundary (IpcError { kind: ErrorKind::from(kind), .. }) — one conversion
    Cancelled,
}
```

### Spawn & progress channel `[DECIDED]`

- Spawned on the **Tokio** async runtime (`tauri::async_runtime` / `tokio::process`,
  §0.9 owns the worker pool & concurrency degree).
- **`stdout`/`stderr` handling is per-`ProgressModel` `[DECIDED]`:** for invocations with a
  **streaming** `ProgressModel` (`FfmpegKeyValue`, `VipsStdout`, `InProcessFraction`),
  `stdout`/`stderr` are **streamed line-by-line** and parsed by the §3.5 per-engine
  adapter into normalised progress ticks (FFmpeg `-progress pipe:` key=value;
  LibreOffice has no native progress → §1.11's heuristic; libvips is fast/atomic →
  coarse ticks). For **`ProgressModel::CoarseSpawnDone`** (the ffprobe probe sub-invocation,
  below) §1.7 instead **buffers stdout in full** and passes the **complete buffer** to the
  §3.5.1 adapter's `ProbeOutput` JSON parser — **no line reader is attached** to a
  CoarseSpawnDone stdout (it would corrupt the single-JSON-blob parse). Normalised ticks flow
  to the frontend over the **§0.4.2 `Channel<ConversionEvent>`** as
  `ConversionEvent::ItemProgress` (the wire shape is defined in **§0.4**, not here;
  "ProgressEvent" in §1.11 is the internal projection of that wire variant). `stderr` is
  **captured in full** for exit-classification and for the §7.5 verbose/diagnostic echo, and
  fed to §2.13 for `stderr`-classify-into-§2.8.
- **Timeout / hang policy:** an item that produces **no progress and no output**
  for a per-engine watchdog interval (parameters owned by §0.9; mechanism here) is
  treated as hung → killed → `Failed(EngineHang)` (§2.8). A hang fails **that one item**;
  the batch continues (SSOT *Fail clearly*).
- **Two-step probe-then-encode (video) `[DECIDED]`:** a video job is **two sequential
  sub-invocations of the one FFmpeg engine** — `ffprobe` then `ffmpeg` — **not** a format
  chain (§3.2.1). Because `Engine::plan()` is **Pure** (no I/O) but the encode argv depends
  on the probe's inner-codec result, §1.7 uses the §3.2.1/§3.2.2 **two-phase trait
  contract**: it **calls `Engine::plan()` (which returns the `ffprobe` sub-invocation),
  spawns it, parses its stdout into a typed `ProbeOutput` (inner codecs + `duration_us` +
  rotation + interlace), then calls `Engine::plan_encode(job, out_tmp, &probe)` to get the
  finalised encode `Invocation`, then spawns the `ffmpeg` encode**. The encode's
  `ProgressModel::FfmpegKeyValue { duration_us }` is built **from `probe.duration_us`
  inside `plan_encode`** — **not** mutated onto a pre-probe `progress` struct (§3.5.1). So
  for video, `plan()` is the probe and `plan_encode(probe)` is the encode; the encode argv
  is never fixed before the probe. Both sub-invocations are bounded by the same §1.7
  cancel/timeout/group-kill machinery. **Probe Invocation has NO publish artifact
  `[DECIDED]`:** the `ffprobe` sub-invocation carries **`out_tmp: None`** (§3.2.2 —
  ffprobe writes only stdout JSON) and **`progress: ProgressModel::CoarseSpawnDone`** (not
  the FfmpegKeyValue line-reader). So §1.7 **does NOT run the §2.1 atomic-publish or any
  temp cleanup for the probe** — it publishes/cleans **only** for an Invocation whose
  `out_tmp.is_some()` (the encode). The probe's only output is the parsed `ProbeOutput`
  handed to `plan_encode`; there is no `*.part`, hence nothing for the §2.6 sweep or the
  cleanup table to handle on the probe leg. (§3.2.1 / §3.5.1 own the sequencing rationale.)
  **Probe stdout is BUFFERED-and-JSON-parsed, NOT routed to the line reader `[DECIDED]`:**
  the probe sub-invocation runs `ffprobe -print_format json …`, which emits a **single JSON
  blob** (not key=value progress lines). So for the probe invocation §1.7 **captures stdout
  in full and hands the complete buffer to the §3.5.1 adapter's `ProbeOutput` JSON parser** —
  it does **not** feed probe stdout to the line-by-line progress reader. The line-by-line
  progress reader (above) is used **only** for invocations with a streaming `ProgressModel`
  (`FfmpegKeyValue` for the encode, `VipsStdout` for the image-worker); the probe's
  `CoarseSpawnDone` model emits a start→done tick while its stdout is buffered for the parser.

### Cancellation / kill mechanism `[DECIDED — sole owner]`

This is the load-bearing, single-owner decision. It must satisfy four SSOT/spec
constraints simultaneously: (a) cancelling keeps already-finished items and
**cleanly discards the one in progress with no partial leftover** (SSOT *Visible
progress, cancellable* + §2.1 no-partial); (b) a decoder crash/hang fails one item
without wedging the app (§2.12); (c) **never touches originals** (§2.4); (d) works
on **Windows, macOS and Linux** with engines that themselves spawn children.

**Problem (researched, real):** several bundled engines spawn **child processes of
their own** — most importantly **LibreOffice**, where `soffice` re-execs/launches
`soffice.bin`; FFmpeg/poppler/pandoc are simpler but must still die promptly.
Killing only the **immediate** child (the naive `std::process::Child::kill`, and
notably Tauri's `tauri_plugin_shell` `CommandChild::kill`, which targets only the
direct child and on Windows is documented to leave the tree running) **orphans**
`soffice.bin` and any decoder grandchildren — leaking processes, file handles and
scratch files, and violating "cleanly discards the one in progress."

**Decision:** ConvertIA spawns every engine as a **process-group / job-object
leader** and kills the **whole group**, so one cancel/kill tears down the engine
*and all its descendants* atomically.

- **Mechanism:** wrap each spawn with the **`process-wrap`** crate (cross-platform
  process-group / Windows Job-Object creation for engine-tree teardown — the
  maintainer-described **successor to `command-group`** by the same author, carrying much
  of its code; versioning starts at 6.0.0 and the paradigm shifts to **composable
  per-concern wrappers** rather than command-group's single cross-platform API),
  composed over `tokio::process`:
  - **Windows:** `JobObject` wrapper — the engine and all its children join one
    **Win32 Job Object**; killing the job (or closing its last handle with
    kill-on-close) terminates the entire tree immediately. Use the crate's
    `KillOnDrop` / `CreationFlags` shims so the job correctly tracks
    kill-on-drop + `CREATE_SUSPENDED`/`CREATE_NEW_PROCESS_GROUP` flags.
  - **POSIX (macOS/Linux):** `ProcessGroup::leader()` wrapper — the engine becomes
    a **process-group leader** (`setpgid`); `kill()` signals the whole group
    (negative-pgid `SIGKILL`/`SIGTERM`), reaping descendants.
  - This deliberately **does not route engine spawning through
    `tauri_plugin_shell`'s sidecar** kill path (whose `CommandChild::kill` is
    tree-incomplete). Per §0.10/§3.3.3 `[DECIDED]` there is **no `shell:allow-execute`
    grant at all** and the shell plugin is **not** used for engine execution: bundled-
    binary **paths are resolved Rust-side** via `current_exe()` / the Tauri
    `PathResolver` (§3.3.3), not the shell plugin's allowlist. The **only** way to
    start an engine is the typed **C6** command the core validates; the spawn+kill is
    pure Rust via `process-wrap` (Windows Job Object / POSIX process-group) for
    tree-correctness. `[DECIDED]` — §0.9/§3.5 align on this one spawn path.
- **Cooperative vs forceful:** v1 uses **forceful group-kill** (no cooperative
  drain). Rationale: these engines have no clean "abort" IPC, the output is
  written to a **temp path** (§2.14) and only atomically promoted on success
  (§2.1), so a hard kill leaves **only** a discardable temp artifact — exactly what
  §2.6 cleanup removes. A graceful `SIGTERM`-then-`SIGKILL` escalation on POSIX is
  a possible refinement but unnecessary for correctness; Windows has no `SIGTERM`
  anyway (job-kill is the primitive). `[REC]` forceful group-kill in v1.
- **Ordering (kill ↔ cleanup ↔ no-partial):**
  1. Signal cancel via the `CancellationToken`; the invocation loop stops reading
     progress.
  2. **Group-kill** the engine and wait for the OS to confirm the group is gone
     (so no descendant still holds the temp file open — matters on Windows where an
     open handle blocks deletion). **Bounded wait `[DECIDED]`:** this confirm-wait is
     **timeout-bounded** (a short cap, generous enough for normal teardown) so a wedged
     descendant — e.g. one blocked in uninterruptible kernel I/O on a dead mount —
     **cannot hang the UI / quit path** (SSOT *app stays responsive*). On timeout the
     item is marked `Cancelled`/`Failed`, its temp reclamation is **deferred to the §2.6
     sweep** (the publish temp is a run-owned `*.part` dotfile, safe to reclaim later)
     rather than blocking on the stuck handle, **and the item carries a `CleanupResidue`
     so the deferred temp is surfaced honestly** — a Cancelled item gets the §2.8.2
     "With residue" summary tail (§2.6.4 case 3), never a silent leftover. This bound is
     what keeps §7.3.3 quit-while-converting from hanging on an unkillable descendant.
  3. **Then** invoke §2.6 cleanup to remove the per-job temp artifact. Because the
     final output is promoted only by the §2.1 atomic rename **after** a clean exit,
     a killed job has **no visible output** to undo — only the temp to discard.
  4. Mark the job `Cancelled` (user) or `Failed(EngineHang/EngineCrash)` (watchdog/exit) and
     **continue the queue** (§1.9). Already-`Succeeded` items are untouched.
- **Granularity:** cancel is **batch-level** in the UI (SSOT "batches … can be
  cancelled"). Internally it maps to: stop dequeuing `Pending`, group-kill the
  currently-`Running` item(s) (≤ the §0.9 concurrency degree), leave `Succeeded`
  intact. A cancelled-but-already-finished item stays finished.
- **App-exit / quit-while-converting:** the same group-kill runs for every live
  job on shutdown (so no orphans survive the app — the §7.3 quit-while-converting
  policy calls into this). On an **ungraceful** end (crash/power-loss) the OS
  reaps the Windows job; POSIX orphans are reaped by re-parenting + the
  **startup cleanup** (§2.6) discarding the previous run's owned temp.

### `InProcessNative` sub-case — the one non-subprocess engine `[DECIDED]`

The lifecycle above is written for **`EngineProgram::Subprocess`** engines (the spawn→
Running→group-kill machine). ConvertIA's only **`EngineProgram::InProcessNative`** engine
is the **native CSV/TSV** transform (§3.5.6, pure memory-safe Rust, no spawn). It has **no
process to kill**, so §1.7 defines its lifecycle explicitly:
- **Progress IPC — self-reported, no line-reader `[DECIDED]`:** because this engine has
  **no stdout to parse**, its `Invocation` carries `progress: ProgressModel::InProcessFraction`
  (§3.2.2). For this variant §1.7 **does NOT attach the §1.7 stdout/stderr line-reader**;
  instead it constructs a **bounded `tokio::sync::mpsc::Sender<f32>` (`progress_tx`)** and
  passes it into the executor when it dispatches the §3.5.6 transform on the
  `spawn_blocking` pool. The synchronous loop calls `progress_tx.blocking_send(fraction)`
  with `fraction = bytes_processed / source_size` at **each N-KB chunk boundary** (the same
  granularity as the cancel poll below). §1.7 owns the matching **`Receiver<f32>`** on the
  Tokio runtime and forwards **every received fraction as one normalised
  `ConversionEvent::ItemProgress` tick** over the §0.4.2 channel — the same
  `{ runId, itemId, fraction, stage }` wire shape every other engine produces (§1.11), so
  the frontend cannot tell this engine apart. A bounded channel applies natural
  back-pressure (a slow consumer just coalesces; no unbounded memory). For **sub-100-KB
  inputs** the loop sends a single `1.0` on completion → an honest start→done tick
  (§1.11), wire-indistinguishable from `CoarseSpawnDone`. Channel close (loop end or drop
  on cancel) ends the forwarding task.
- **Cancellation (cooperative, not a kill):** the synchronous streaming loop **polls the
  job's `CancellationToken` at every N-KB chunk boundary** (the same chunk granularity it
  uses for its `bytes_processed / source_size` progress, §1.11). On cancel it **stops
  mid-stream, drops the `out_tmp` `TempPath`** (deleted on drop, §3.2.2) and reports
  `Cancelled` — exactly the "no partial leftover" guarantee, reached cooperatively instead
  of by group-kill. There is **no kill step to sequence** in the §2.6 ordering for this
  engine (step 2 "group-kill" is a no-op; the temp-discard step still runs).
- **Timeout / hang bound:** a **wall-clock timeout guard** (the §0.9-owned per-engine
  timeout, tight for this light engine) wraps the synchronous call; on expiry the loop is
  cancelled cooperatively (same chunk-boundary poll), the temp is discarded, and the item
  is `Failed(EngineHang)` — so even a pathological input cannot wedge a worker forever.
  - **Wedged-uninterruptible-read caveat `[DECIDED]`:** because this engine has **no
    subprocess to force-kill**, a read that **blocks uninterruptibly in the kernel** (e.g. a
    dead network mount, a stalled USB) **cannot be force-cancelled** — the cooperative poll
    only fires at a chunk boundary, and a wedged read never reaches the next boundary. In that
    case the **timeout marks the item `Failed(EngineHang)` and the run CONTINUES** (the wedged
    thread is abandoned, not awaited), exactly like the subprocess hang case — the user is
    never left staring at a hang. **The abandoned thread MUST NOT exhaust the blocking pool
    `[DECIDED]`:** the `spawn_blocking` pool is **bounded** (a few parked threads cannot starve
    it — the pool size is sized with headroom above the global degree), AND/OR CSV/TSV reads go
    through a **bounded chunked reader with a short per-read deadline** so a single read syscall
    cannot block indefinitely in the first place. Either way a handful of wedged reads degrade
    gracefully (those items fail, the batch finishes) rather than wedging the whole pool.
- **Concurrency / permit model:** it runs on the §0.9 pool **up to the global degree, on
  dedicated worker threads** (a `spawn_blocking`-style pool so the synchronous CPU/IO loop
  **never blocks the Tokio runtime** that drives the subprocess engines and the IPC). It
  holds a global-degree permit like any other job; it has **no** `serialised_only` lane.
  The §1.10 input-size guard bounds CSV expansion (a §2.12.4 in-core untrusted-byte path,
  but pure bounded Rust — DoS-bounded, see §2.12.4).

### Exit & output verification `[DECIDED]`

On exit 0, the adapter (§3.5) reports success **only if** the expected temp output
exists and is non-empty (a "success exit but empty/zero output" — e.g. an
essentially-empty PDF→TXT extraction — is handled per the 04 edge rules / §2.8, not
reported as a clean success of an empty file). Exit ≠ 0 or a `stderr`-classified
fault maps to the **§2.8 error taxonomy** (corrupt / unsupported-internal /
password-protected / engine-crash / …); the mapping table is §2.8's, fed by §3.5's
per-engine classifier.

---

## 1.8 Output planning `[DECIDED]`

The pipeline computes the **`OutputPlan`** for each job **before** the write,
applying the **rules owned by §2.7** (beside-each-source default; user-chosen
destination re-creates the relative subtree; per-location divert for
unwritable/ephemeral locations). The §2.1 atomic write (with the §2.14
cross-volume strategy) **consumes** this plan.

```rust
/// Computed per job, before any write. Rules = §2.7; naming = §2.2; identity = §2.3.
/// This is the canonical shared/wire shape, copied verbatim into §0.6 (domain model)
/// and consumed by §2.1/§2.14. DIRECTORY-BASED by design: the exact final name +
/// no-clobber numbering is resolved at write time on the RESOLVED real file
/// (§2.1 exclusive-create) — NEVER pre-baked into a `final_path` string (a
/// pre-numbered path would reintroduce the §2.1.2 TOCTOU race).
struct OutputPlan {
    job: JobId,
    final_dir: PathBuf,          // beside-source OR diverted (§2.7)
    diverted: Option<DivertReason>, // unwritable / ephemeral (§2.7); None = beside-source
    base_name: OsString,         // SOURCE base name kept (§2.2)
    extension: OsString,         // from the chosen TARGET (§2.2)
    publish_temp_dir: PathBuf,   // where the kind-1 publish temp lives. EQUALS final_dir in
                                 //   v1 (§2.14.1): the `*.part` is a uniquely-named sibling
                                 //   DOTFILE inside final_dir, NOT a per-run scratch subdir.
                                 //   Same volume as final_dir by construction (§2.14.1). Distinct
                                 //   from the kind-2 engine-working scratch root (§2.14.2), which
                                 //   lives under app_local_data_dir and MAY be on another volume —
                                 //   that root is NOT carried in OutputPlan (it is run-scoped, §2.6).
    // No `crosses_volume` field: the PUBLISH detects cross-volume REACTIVELY on EXDEV at
    // publish time via `fs_guard::atomic_publish` (§2.14.3). "Not pre-planned" = no plan
    // FIELD, NOT "no pre-engine decision": where the engine writes when a same-volume
    // sibling temp can't be created is a pre-engine temp-PLACEMENT decision owned by
    // §2.14.3 at run time (not stored here). (§0.6 invariant 5.)
}
// (DivertReason, JobId == ItemId: §0.6.)
```

What this section **owns**: the *computation* — resolve the destination root,
re-create the dropped-root-relative subtree (so folder layout is preserved /
re-created rather than flattened), choose the divert target when a location is
unwritable/ephemeral, and set `publish_temp_dir` (= `final_dir` in v1; the `*.part` is
a sibling dotfile there, not a subdir, §2.14.1) on the **same filesystem as `final_dir`**
so the §2.1 rename stays atomic (cross-volume fallback owned by §2.14). What it **references**: the *rules* (§2.7), the *naming contract* (§2.2),
the *resolved-identity & link safety* (§2.3), and the *atomic write itself* (§2.1).

**Destination shown before convert (SSOT How It Feels 7):** the `OutputPlan`
(specifically `final_dir`, and any divert) is computed early enough (in **C4
`plan_output`**, §0.4.1) to render the "will save to …" line **before** conversion
starts and to let the user change the destination. A user-chosen destination
(**C5 `set_destination`**, §0.4.1) **revalidates writability and updates the held
destination authority + the §1.10 preflight verdict** for the new volume (§2.14.4) — it does
**not** itself re-create the relative subtree. The per-job `OutputPlan` re-computation that
actually applies the new destination's **relative-subtree re-creation (§2.7)** runs at
**C6/write-time**, using C6's authoritative `destination` argument (§0.4.1 C6 destination
authority). So C5 = revalidate + hold the destination + refresh the preview verdict; C6 =
build the OutputPlan (with subtree re-creation) from the held destination.

**Eager C4 `location_status` vs lazy at-write probe — reconciled `[DECIDED]`.** C4's
divert classification and the §1.10 per-volume preflight need each item's `final_dir`
volume + its writable/ephemeral verdict, which come from §2.7.2 `location_status`. C4
therefore runs `location_status` **per UNIQUE intended destination directory, cached**
(not once per file): a many-files-in-few-folders batch (e.g. 10 000 files in one folder)
probes that folder **once** and reuses the cached verdict for every item under it, so the
eager-at-C4 cost is bounded by the number of distinct destination directories, not the
file count. This is **consistent with** §2.7.2's "probe lazily and cache per-directory":
the C4 probe is the **planning hint** (classifies divert + resolves the preflight volume),
and §2.7.2's **at-write re-check** (the late-divert path) is the authority if a location
flips read-only between C4 and the actual §2.1 publish — the cached verdict is never a
commitment, only a hint (§2.7.2). So C4 is eager-per-directory (cheap, cached) and the
write is the lazy re-check — no contradiction.

**Destination-change re-validation `[DECIDED]`:** because the §2.14.4 free-space
check targets the **destination** volume, a C5 destination change must **not** leave
the held C4 free-space verdict stale. C5 therefore returns a **re-evaluated
`PreflightVerdict`** alongside the resolved destination — i.e. C5 re-runs the
destination-dependent slice of C4's planning for the new volume (§0.6
`DestinationResolved`; §0.4.1 owns the wire shape). The **§2.5 re-run verdict is
destination-INDEPENDENT in v1** (the EquivKey has no destination component, §2.5.1),
so `rerun` is **carried through unchanged** from C4 — C5 does not recompute it. The
plan itself is recomputed per job at write time from the updated destination.

---

## 1.9 Job & batch lifecycle `[DECIDED]`

### States

```rust
// The state TYPE is §0.6's `JobState` (referenced, not redefined); this section
// owns the TRANSITIONS between its variants. For convenience the variants:
//   Pending                  // queued, not started
//   Running                  // engine invoked (1.7)
//   Succeeded                // output verified + atomically published (§2.1)
//   Failed(ErrorKind)        // named §2.8 kind (see Running→Failed mapping below);
//                            //   nothing written for it
//   Cancelled                // user cancel; nothing written for it
//   Skipped(SkipReason)      // detected-ineligible pre-flight (§0.6 SkipReason:
//                            //   UnsupportedType | Uncertain | Empty | Unreadable)
```

```
                 ┌────────────▶ Skipped        (set at detection/grouping; never enters the queue)
Pending ─▶ Running ─┬─▶ Succeeded
                    ├─▶ Failed(kind)
                    └─▶ Cancelled
```

- `Skipped` is assigned **before** the queue (a §1.2/§1.3 ineligible item never
  becomes `Pending`); it is distinct from a mid-run `Failed`.
- **Running → Failed mapping — where the kind conversion lives `[DECIDED]`.** When the
  §1.7 lifecycle returns `InvocationResult::Failed(kind)` (carrying the Rust-internal
  `ConversionErrorKind`, §2.8), the orchestrator — **`crate::run`**, which owns the
  transition — advances the job to `JobState::Failed(...)` by mapping the internal kind
  to the wire `ErrorKind` **immediately, before** the state is recorded / a
  `RunResult.items` row or live `ItemFinished` event is emitted:
  ```rust
  // in crate::run, on InvocationResult::Failed(kind):
  let wire: ErrorKind = ErrorKind::from(kind);   // From<ConversionErrorKind> for ErrorKind
  job.state = JobState::Failed(wire);            // §0.6 JobState
  ```
  The `From<ConversionErrorKind> for ErrorKind` impl is **owned by `crate::outcome`**
  (the §2.8 taxonomy ↔ §0.4.3 IpcError mirror module), **not** `crate::run`. Under the
  §2.8 **preferred** anti-drift mechanism (`ErrorKind` is a *type alias* for
  `ConversionErrorKind`) the `From` is the identity blanket impl and the map is a no-op
  cast; under the two-enum fallback it is an explicit per-variant `match` in
  `crate::outcome`. Either way the conversion site is **`crate::run`** and the conversion
  *definition* is **`crate::outcome`** (cross-ref §0.4.3 / §1.7 / §2.8). The same
  `ErrorKind::from(kind)` is what §1.7's `InvocationResult` comment and §0.4.3's IPC
  boundary refer to — one conversion, named once.
- A `Batch` aggregates its jobs; the batch is `Running` while any job is
  `Pending`/`Running`, then resolves to a summary (§1.12).

### Queue semantics `[DECIDED]`

- **Ordering:** deterministic, stable — recommended **collected/traversal order**
  (depth-first folder order from §1.1), so progress and the summary read
  predictably. `[REC]` no priority/size reordering in v1.
- **Concurrency:** the number of jobs `Running` at once = the **§0.9 concurrency
  degree** (referenced, not set here). §0.9 also owns the **LibreOffice
  serialization** rule (headless LO is *not* safely parallel under one user
  profile — parallel instances lock/corrupt). The pipeline simply respects the
  degree and any per-engine serialization the §0.9 pool enforces; it does not pick
  the number.
- **Batch construction projects pre-flight skips as non-queue `Skipped` records `[DECIDED]`.**
  When the orchestrator (`crate::run`) builds the `Batch` from the **frozen `CollectedSet`**
  at **C6 (start_conversion)**, it creates, for **every `SkippedItem` in
  `CollectedSet::Single.skipped`**, a `ConversionJob` record with
  `JobState = Skipped(reason)` set **at construction** (the `SkipReason` copied directly from
  `SkippedItem.reason`, §0.6). These `Skipped` jobs **never enter the `Pending` queue** and
  receive **no `Channel` events** (no live `ItemStarted`/`ItemProgress`/`ItemFinished`, per
  §0.4.2) — they exist **only as non-queue entries** so the §1.12 run-end projection can emit
  them into `RunResult.items` and `Totals.skipped`. A `Skipped(reason)` job **never
  transitions** (it is terminal at construction). This is the single anchor that prevents a
  skip from being stored only inside the `CollectedSet` and lost at C6: the skips are
  materialised into the `Batch` at construction, alongside the `Pending` eligible jobs, over
  the §1.1 single id space (so a `SkippedItem.item` never collides with an eligible `ItemId`).
- **Per-item isolation:** each job runs through its own §2.12-isolated invocation
  with its own per-job scratch (§2.6 ownership). A **worker-thread panic** is
  caught at the §2.13 panic boundary and surfaced as a clean per-item `Failed`,
  never a poisoned pool that wedges the batch.
- **Mid-run skip vs pre-flight refusal (the SSOT distinction, restated):**
  - **Pre-flight refusal** (§1.3 `Mixed`): a *multi-format drop* is rejected
    **wholesale, before converting** — re-drop a single format.
  - **Mid-run skip** (here): a source that was present at drop but is **unreadable
    or gone when its turn comes** (removed media, moved/deleted/renamed, exclusive
    lock, denied read), or a corrupt/too-big/out-of-disk item, fails **that one
    item** with a §2.8 message and the **batch continues**. A bad item is **never
    silently** dropped — it appears in the summary.
- **A batch where everything failed is a clear failure**, never a quiet finish
  (§1.12).

---

## 1.10 Resource pre-flight & budgets `[DECIDED design; DEFER: corpus-tuned numbers]`

The model the SSOT Boundary Note delegates ("disk-space estimation thresholds,
very-large-batch handling"). This section owns the **estimation + decision**;
concrete per-pair size heuristics are supplied by 04 (e.g. the GIF guardrail in
`cross-category.md`), and the **concurrency degree** is §0.9.

### Up-front estimation `[REC]`

Before (and during) CONVERT, estimate **output + scratch footprint** per item, so a
doomed run fails **fast and clearly, preferably up front** (SSOT How It Feels 6):

```rust
struct SizeEstimate {
    est_output_bytes: u64,       // output + kind-1 publish temp → the item's final_dir VOLUME (§2.14.1)
    est_scratch_bytes: u64,      // kind-2 engine working temp → the system/scratch VOLUME (§2.14.2),
                                 //   NOT necessarily the destination volume — checked separately per
                                 //   physical volume (§2.14.4 / the per-physical-volume preflight below).
                                 //   ON macOS this ALSO INCLUDES the Σ of staged input sizes (the
                                 //   §3.5.0/§7.2.6 TCC source-into-scratch copy, input-sized per
                                 //   in-flight item); on Windows/Linux that term is 0 (no TCC staging).
    basis: EstBasis,             // PerCategoryHeuristic | EngineProbe
}
```

- **Per-category heuristic** (cheap, no decode): e.g. images ≈ source-pixels ×
  bytes-per-pixel for the target codec; **GIF** uses the explicit `frames × w × h ×
  ~1 byte/px` guardrail (supplied by `cross-category.md` [OPEN-F]); audio/video
  bounded by source size/duration. The heuristic **constants** are co-owned with
  04 and **must be finite** (a missing cap reintroduces the foot-gun).
- **Where the cheap estimate's inputs come from `[DECIDED]`** (so it never needs the
  convert-time `ffprobe`):
  - **Raster images:** §1.2 detection **carries header-derived `width`/`height`** in
    `DetectionOutcome::Recognized.dims: Option<(u32,u32)>` (the dimensions sit in the
    format header its bounded structural-peek already reads — JPEG SOF, PNG IHDR, etc.),
    so per-pixel estimates consume `dims`, no decode. When `dims` is `None` (header
    lacked them) the estimate falls back to the source byte-size bound like video below.
  - **Video / GIF:** the cheap pass does **NOT** run a per-item `ffprobe`; it uses a
    **worst-case bound from source byte-size** (+ the GIF duration cap from
    `cross-category.md`) — deliberately conservative. The precise per-item
    duration/dimension probe (`EstBasis::EngineProbe`) is **deferred to convert-time**
    (§3.5's `ffprobe`, which runs then anyway), where a refined estimate may still trip
    the mid-run enforcement. So `PerCategoryHeuristic` is the up-front basis; `EngineProbe`
    is the convert-time refinement, never an up-front cost. (Aligns the cross-category
    `[OPEN-C]`.)
- **Headroom margin:** require **free space ≥ footprint × margin** on **each physical
  volume** (see the split below — `est_output` and `est_scratch` may land on different
  volumes). `[REC]` margin **1.3×** as a starting value (confirm against the §6 corpus).
- **Decision (the up-front-vs-mid-run split, made precise) `[DECIDED]`:**
  - **Whole-batch doomed is PER-PHYSICAL-VOLUME, split by where each byte lands `[DECIDED]`.**
    The §2.7 beside-source default lands each item's **publish temp + final** on its **own
    source volume** (§2.14.1), and per-location divert sends some items to Downloads and
    others beside themselves — so a batch routinely spans **2+ destination volumes with no
    single destination volume**. Crucially, the **kind-2 engine working scratch** (LO
    per-run profile, FFmpeg two-pass/internal temp — `est_scratch_bytes`) does **NOT** land
    on the destination volume: it lands on the **system / scratch volume** that
    `app_local_data_dir()`/`temp_dir()` resolves to (§2.14.2). A summed check against one
    volume is therefore **wrong** in two ways (a 5 GB share destined for a 1 GB USB stick
    falsely PASSing against 500 GB internal; a heavy office batch exhausting the **system**
    volume while every destination volume passes). Instead, group by **physical volume,
    split by category**:
    - **`est_output_bytes` + the publish temp → each item's `final_dir` volume** (the
      destination volume — computable in C4 after the §2.7 divert classification).
    - **`est_scratch_bytes` (kind-2) → the system/scratch volume** (§2.14.2) — **on macOS
      this includes the staged input sizes** (§3.5.0/§7.2.6 TCC source-staging copy,
      input-sized per in-flight item); on Windows/Linux that term is 0. **The macOS
      staged-input term is bounded to PEAK CONCURRENT footprint, NOT the whole-batch Σ
      `[DECIDED]`:** staged source copies are **reclaimed per-item** (the staged copy is
      freed as soon as that item's engine finishes, §2.14.2) and at most the §0.9
      **concurrency degree** of them coexist, so summing **every** item's staged-input size
      across the whole batch grossly over-counts the simultaneous footprint and could falsely
      trip `up_front_fail = OutOfDisk`. The correct staged-input term is the **peak
      concurrent footprint** ≈ `degree × (largest staged inputs among the in-flight set)`
      (conservatively, the `degree` largest source sizes in the batch), per §0.9's
      bounded-concurrent-decodes invariant. The LO/FFmpeg working-space part of
      `est_scratch_bytes` is likewise a concurrent-degree footprint, not a whole-batch sum.
    Sum the **destination-volume** term per physical volume across the batch (publish temp +
    final accumulate until run end), but use the **peak-concurrent** bound for the
    scratch-volume kind-2 term (it does not accumulate across the whole batch); a destination
    volume that *is* the scratch volume gets both. Require headroom on **each** volume
    **independently** (× the margin). Set `PreflightVerdict.up_front_fail = Some(OutOfDisk)` when **any one physical
    volume's grouped footprint cannot fit its free space**. `TooBig` (the absolute
    output-size ceiling) stays per-item / aggregate as before. This is the **only** up-front
    fail carrier — batch-level by design, but evaluated per-physical-volume (destination
    volumes **and** the system/scratch volume).
  - **Per-item too-big / out-of-disk** is **enforced at WRITE TIME (mid-run)**: when an
    item's own size/space breaches the budget (or real disk usage outruns the estimate),
    its §2.1 write fails, §2.6 restores free space, and the item is reported as
    `Failed(TooBig|OutOfDisk)` (§2.8) **while the batch continues** (§1.9/§1.11 fast-fail
    surfacing). There is **no** per-item up-front-fail list on `PreflightVerdict`; a
    per-item doom shows as that item's mid-run terminal row, not a pre-convert verdict.
  - So: **estimate up front; the per-volume whole-batch doom fails up front; per-item doom
    is enforced at the write** — the SSOT "preferably up front" is honoured by the
    per-volume whole-batch verdict (which now correctly catches the doomed-USB-volume case
    in the common beside-source layout), and per-item correctness is honoured at write time.

### Ceilings & large lists `[DECIDED design; DEFER: corpus numbers]`

- **`[DEFER: corpus]` (owner §1.10, co-owned §0.9 + 04)** the concrete numbers: the
  absolute **"too big" output ceiling** (**starting values `[DECIDED design]`: ~4 GB
  per-item projected output, ~16 GB aggregate-batch projected output** — finite from day
  one so `TooBig` is enforceable, calibrated against the corpus), the **memory/handle
  ceilings**, the per-category heuristic constants, the **headroom margin (1.3× starting
  value)**, and the **GIF duration cap (~10 s starting value)** (`cross-category.md` [OPEN-F]).
  These are **genuinely empirical** — the right thresholds depend on corpus
  timing/measurement (a §6 asset), so they are **deferred to corpus calibration**,
  not left open as a design question. They ship with the stated finite starting
  values (margin 1.3×, GIF cap ~10 s) and are tuned against the real-world corpus
  (SSOT *v1 DoD* reliability gate) — finite-from-day-one, calibrated-against-corpus.
- **Large recursively-collected lists** (thousands of files): the **frozen set and
  job queue are bounded in memory** by storing lightweight `ItemId`/path records,
  not file contents; the **UI list is virtualized** (§5 owns the virtualization
  component). `[REC]` no hard cap on file *count* in v1 (the cap is on per-item
  size and total disk, not list length); a very large batch simply queues and shows
  aggregate progress (§1.11). Memory stays flat because only ≤ the §0.9 concurrency
  degree of items are decoded at once.
- **Low-memory graceful degradation `[DECIDED design; DEFER: corpus]`.** On a
  memory-constrained machine the app **degrades — it never OOM-crashes or freezes**: the
  **effective §0.9 concurrency degree adapts to available memory** (down to **serial**:
  `effective = min(cpu-degree, per-engine-cap, memory-based-cap)`), a **high-memory
  watermark pauses dispatch of NEW items** (in-flight items finish; the §5 passive `LowMemoryNote` banner
  shows a brief "working — low memory" line, not a modal) and resumes as memory frees, and
  a single item that still exceeds its **§1.10 per-item memory ceiling** is killed (the §1.7 kill mechanism, reinforced by the §2.12.3 Job-Object memory cap where that tier is present) to a clean
  `Failed(TooBig)` (the batch continues, host RSS returns to baseline). The watermark + the
  memory-based degree cap are corpus-calibrated starting values (like the other §1.10
  numbers). This is why the §0.3.1 2 GB floor holds: bounded concurrency + adaptive degree
  + per-item kill keep peak RSS finite regardless of batch size.

This section **feeds** §1.8 (plan only if it fits), §2.6 (cleanup on
out-of-disk), §2.8 (the named failure kinds), §2.14 (scratch sizing) and §5
(virtualization + the fast-fail message).

---

## 1.11 Progress & cancellation `[DECIDED]`

### Real per-item progress (not indeterminate) `[DECIDED]`

Every item reports a **real progress fraction**, never an indeterminate spinner
(SSOT *Visible progress, cancellable* — "working, not hung"), even for a single
long conversion. The fraction source per engine (parsed by §3.5, normalised by
§1.7, delivered over the §0.4 channel):

| Engine | Progress basis |
|--------|----------------|
| **FFmpeg** (audio/video/cross-cat) | `-progress pipe:` → fraction = **`out_time_us` / source-duration-µs** (the denominator is the **`ffprobe` source duration**, NOT `total_size` — `total_size` is FFmpeg's running *output byte count*, which is not a duration and must not be the denominator) → true % even for a 2-hour film |
| **image-worker** (libvips, images) | `ProgressModel::VipsStdout` (§3.2.2): the separate image-worker process marshals libvips' `eval`-progress signal to its **stdout** as `progress=<0..100>` key=value lines (it cannot deliver an in-process callback across the process boundary), parsed by the §1.7 same stdout reader as FFmpeg's `-progress`. Fast ops emit start→`progress=end` (coarse); HEIC/AVIF HEVC/AV1 encode reports a real % |
| **LibreOffice** (office/PDF) | No native progress signal → a **bounded indeterminate-but-animated** state with a watchdog (still reads as "working"); `[REC]` show a determinate-looking staged bar driven by the **four canonical §0.6 `JobStage` values — `Spawning` → `Decoding` → `Encoding` → `Writing`** rather than a raw spinner. (The LO lifecycle maps onto them: process spawn / profile init → **`Spawning`**; load+layout the source document → **`Decoding`**; run the export/render filter → **`Encoding`**; flush the produced file → **`Writing`**. No separate "render"/"export" stage vocabulary is emitted on the wire — only the four `JobStage` names §0.6 defines, so §1.11 and §0.6 agree on what the frontend receives.) |
| **poppler / pandoc** | Usually fast; staged ticks; large PDFs report per-page where `pdftotext` allows |
| **Native CSV/TSV** (in-process, §3.5.6) | `[DECIDED]` `ProgressModel::InProcessFraction` (§3.2.2): fraction = **`bytes_processed / source_size`** emitted per N-KB chunk as the in-process engine streams the file (there is no subprocess to watch, so it **self-reports** over the §1.7 `InProcessNative` `progress_tx: mpsc::Sender<f32>`, which §1.7 forwards as `ItemProgress`). For **sub-100 KB inputs** it is effectively instant → a single **start→done** tick (wire-indistinguishable from `CoarseSpawnDone`). So even the only non-subprocess engine reports a real fraction (or an honest start→done for tiny files), never a bare spinner. |

```rust
// payload SHAPE is §0.4.2's `ItemProgress` { runId, itemId, fraction, stage };
// `JobStage` is the §0.6 wire enum (Spawning | Decoding | Encoding | Writing).
// This section owns the SEMANTICS (the per-engine fraction basis above):
//   fraction: Option<f32>   // 0.0..=1.0 ; None ONLY where truly unknowable (LibreOffice)
//   stage:    JobStage      // §0.6
// For the None-fraction LibreOffice case the frontend synthesises a staged
// determinate-looking bar from `stage` transitions (§5.3) — never a raw spinner.
```

**Video probe-phase progress gap `[DECIDED]`.** A video job runs the `ffprobe`
sub-invocation (`ProgressModel::CoarseSpawnDone`, §1.7) **before** the encode's
`FfmpegKeyValue` fraction starts, so without a deliberate tick the bar would sit at **0%**
during the probe (a "looks hung" moment, contra SSOT *working, not hung*). So §1.7 emits, for
the probe leg: **`ItemProgress { fraction: Some(0.0), stage: Spawning }` at probe-start** and
**`ItemProgress { fraction: Some(0.05), stage: Decoding }` at probe-done** — the bar shows
immediate motion (0 → 5%) while the probe runs, then the **encode `FfmpegKeyValue` fraction
takes over from 0.05 onward** (rescaled into the 0.05..=1.0 band, or simply continuing — the
encode % dominates the runtime). This is the **one deliberate departure** from the
`None`-fraction LibreOffice case: the probe is short and bounded, so a small synthetic
start/done pair is honest (it really is spawning then decoding the header), not a fake.

### Aggregate batch progress `[DECIDED]`

The batch shows **both** per-item progress (the active item[s]) and an **aggregate**
(`completed_items / total_items`, with the active item's fraction blended for
smoothness). `[REC]` aggregate = `(succeeded + failed + cancelled + active_fraction)
/ total` — monotonic, never jumps backward.

### Cancellation (surfaced here, mechanism §1.7) `[DECIDED]`

Cancellation is **surfaced** in this section (the batch-level "Cancel" affordance,
optimistic-UI-then-confirmed-kill round-trip owned by §5.8) but the **mechanism is
owned by §1.7** (group-kill, ordering, no-partial). Cancelling **keeps the files
already finished** and **cleanly discards the one in progress** (no partial
leftover, never touches originals).

### Fast-fail surfacing `[DECIDED]`

"Too big / doomed for disk space" items (decided by §1.10) surface here as an
immediate per-item fast-fail (preferably up front), with the §2.8 message, while
the rest continue. The app **stays responsive regardless of batch or file size**
(all conversion is off the UI thread, on the §0.9 Tokio pool) — **including on a low-RAM
machine**, where the §1.10 low-memory policy reduces the effective degree / pauses
new-item dispatch rather than thrash or freeze the UI.

---

## 1.12 End-of-batch summary `[DECIDED]`

When every job has left `Pending`/`Running`, the pipeline emits the **`RunResult`**
summary. **`RunResult`, `ItemResult`, `Totals` and `CleanupResidue` are §0.6
domain-MODEL types — their shape is owned and defined in §0.6, referenced (never
restated) here** (their `crate` module home is `orchestrator`, not `crate::domain`,
per the §0.7 ‡ tier-finalisation, since `RunResult`/`ItemResult`/`ItemOutcome`
reference `crate::outcome`; this
section *computes* them; §0.4.2 carries `RunResult` as the `RunFinished` payload;
§5.3 `ResultSummary` renders it). For reference, the §0.6 shape is:

- `RunResult { collected_set_id, run_id, items: Vec<ItemResult>, totals: Totals,
  cleanup_incomplete: Vec<CleanupResidue>, common_root, divert_root: Option<PathBuf> }`
  (`common_root` = beside-source open-folder target; `divert_root` = `Some(..)` when any
  item diverted, §0.6 / §2.7.4 — a single field cannot carry both roots)
- `ItemResult { source, state: JobState, output: Option<PathBuf>, reason:
  Option<OutcomeMsg> }`
- `Totals { succeeded, failed, cancelled, skipped }` — the "all failed" condition is
  **derived** (`failed == total && total > 0`), not a stored field.
- `CleanupResidue { item, residue_path }` (§2.6.4).

- **Per-item success/failure with reasons** and **output locations**; every output
  is **mapped back to its source** (SSOT How It Feels 7 — the completion summary
  maps each output to its source; matters for flattened/diverted fallback outputs).
- **Fully-failed batch = a clear failure**, never a quiet finish (SSOT *Fail
  clearly*).
- **Cleanup-failure honesty (§2.6):** if cleanup could not complete for an item, it
  is **never reported as a clean success** — the summary says residue may remain and
  **where** (`cleanup_incomplete`).
- **Completion actions** (one-click "open folder" / "open file" — SSOT How It Feels
  8) are rendered by §5.3 `OpenActions` and executed by §7.7; "open folder" opens
  `common_root` (the beside-source root), and — when `divert_root` is `Some(..)` (a
  split-output batch) — a second "open Downloads/Documents" affordance opens the
  `divert_root`. Per-item diverted outputs are also reachable via each `ItemResult.output`
  (C9 `open_path` with `kind = RevealInFolder`, whose Rust handler calls
  `OpenerExt::reveal_item_in_dir`). The summary is the data; the buttons are §5/§7.7.
- **Re-run prompt linkage:** if §2.5 detected an equivalent prior output **before**
  CONVERT, the one batch-level skip/fresh-copy prompt (§5.2) already resolved it;
  the summary reflects whichever the user chose (skipped items appear as a distinct
  outcome, not a failure).
- **Pre-flight skips ARE in `RunResult.items` `[DECIDED]`.** The freeze-time
  `SkippedItem`s held in `CollectedSet::Single.skipped` (the unsupported / uncertain /
  empty / unreadable-at-intake items that **never entered the queue**, §1.1/§1.3) are
  **projected into `RunResult.items` at run-end** as
  `ItemResult { source, state: JobState::Skipped(reason), output: None, reason: Some(OutcomeMsg::Skipped{ reason, .. }) }`
  — a **trivial copy** of `SkippedItem.reason` (a `SkipReason`, §0.6: `UnsupportedType` /
  `Uncertain` / `Empty` / `Unreadable`) into the same-typed `JobState::Skipped(SkipReason)`
  and `OutcomeMsg::Skipped{ reason: SkipReason }` (§2.8) — **no lossy ErrorKind→SkipReason
  reverse map** (the §1.12 helper only ever applies the forward `SkipReason → ErrorKind`,
  e.g. `Uncertain → Unrecognized`, if an ErrorKind-shaped display string is also needed).
  They are **counted in `Totals.skipped`** (never
  `failed`). **The reason rides the skip-shaped `OutcomeMsg::Skipped` variant** (§2.8),
  **not** `OutcomeMsg::Failure` — so a consumer pattern-matching `OutcomeMsg` can tell a
  skip from a fail without also reading `ItemResult.state` (§0.6 keeps `Skipped` and
  `Failed` distinct and §1.12 `Totals` counts them separately; they must not be conflated). This gives the §5.2 Summary UI a single uniform source for every item's
  source path + reason — pre-flight skips and in-run outcomes render the same way — and
  resolves the otherwise-ambiguous "where does the Summary get a skipped item's
  source/reason" question: it is in `RunResult.items`. (The pre-flight skip is **also**
  shown earlier in the §1.4 confirm summary; appearing again in the final summary is
  intentional, so nothing the user dropped is silently dropped, §1.4/§0.6.)

---

## Open items raised by this file (for the README open-questions log)

| ID | Item | Owner | Status |
|----|------|-------|--------|
| 1.10-a | Resource budgets: absolute "too big" output ceiling, memory/handle ceilings, per-category size-heuristic constants, headroom margin (1.3×), GIF duration cap (~10 s) | §1.10 (co-owned §0.9 + 04) | `[DEFER: corpus]` — ship with the stated finite starting values; calibrate against the §6 corpus (design is decided, only the numbers are empirical) |
| 1.2-sec | Whether the in-core text-encoding heuristic / Rust ZIP central-directory peek / **`.svgz` pure-Rust bounded inflate (flate2 `rust_backend`/miniz_oxide, ≤64 KiB + ≤100× ratio cap)** may stay outside the §2.12 isolation boundary | §2.12.4 (raised by §1.2) | **`[DECIDED]` — YES, they stay in-core** (all memory-safe, bounded, no third-party **C/C++** decoder, so they satisfy the §2.12.4 "no C/C++ decoder in-core" absolute). Resolved in the consolidation pass — §2.12.4 / README resolved log. |

### Resolved here with a recommended default (`[REC]`)

- **Engine spawn+kill path** = `process-wrap` (Windows Job Object / POSIX process
  group), **not** the Tauri shell-plugin `CommandChild::kill` path, so the whole
  engine *subprocess tree* (esp. LibreOffice `soffice.bin`) dies on cancel/kill.
  (§1.7 — the sole-owned mechanism; flagged for §0.9/§3.5 alignment.)
- **Forceful group-kill** in v1 (no cooperative drain); safe because output is
  temp-then-atomic-rename (§2.1/§2.14). (§1.7)
- **Kill→wait-for-group-gone→cleanup→continue** ordering (so no descendant holds the
  temp open on Windows). (§1.7)
- **Hidden/system ignore list** = dotfiles + `.DS_Store`/`Thumbs.db`/`desktop.ini` +
  Windows hidden/system attribute (fixed, not user-config). (§1.1)
- **§1.6 owns a CI-generated consolidated defaults registry** validated against 04 for
  the DoD "no required choices" gate. **`[DECIDED]` (escalated from `[REC]`):** §6.7.1
  Lane-A generates the index and fails the build if any §04 pair lacks a default; §6.10
  row 7 reads "owned by §1.6". (§1.6 / §6.7.1 / §6.10) — *no longer a bare `[REC]`.*
- **Queue order** = deterministic collected/traversal order; no reordering. (§1.9)
- **Aggregate progress** = monotonic `(done + active_fraction)/total`. (§1.11)
- **No hard file-count cap**; bound memory via lightweight records + §5
  virtualization + §0.9-bounded concurrent decodes. (§1.10)
- **LibreOffice progress** shown as a staged determinate-looking bar (not a raw
  spinner) since LO emits no native progress. (§1.11)
