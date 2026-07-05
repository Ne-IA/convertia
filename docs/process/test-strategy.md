# ConvertIA — Test Strategy (living)

> **How we write tests for a converter of untrusted files.** The doctrine the
> Build-Loop applies at **step 4 of every box** ([build-loop.md](build-loop.md) §3):
> *which* level a change is tested at, *what* each level owns, and the one bar that
> is non-negotiable for a converter — **output validity proven by a real structural
> reader, never by "the engine returned no error".**
>
> This file is the **process home** for the test methodology; the **technical home**
> is [spec §6.4](../spec/06-build-test-release.md) (`§6.4.1`..`§6.4.6a`) and the
> **enforcement home** is the [build-gate catalogue](../security/build-gates.md)
> (`Gnn`). Where the spec owns a fact (the corpus contents §6.4.5, the reliability
> ledger §6.5, the per-platform driver matrix §6.4.6), it is **referenced by `§`**,
> never restated. Where a gate enforces a rule, the `Gnn` is named.
>
> **Status: living.** Refined *during* implementation; a methodology change is
> recorded here first, in the same commit as the change. **Conflict order
> (unchanged):** SSOT > spec > security/process docs > plan > code > conversation.

---

## 0. The two rules that govern every level

Two rules sit above the level table because they decide more bugs than any single
level does.

### 0.1 Never mock the thing under test

The conversion **is** the product. So the converter, the codec/engine, and the
filesystem are **never mocked** in the test that is supposed to prove conversion
works. A test that swaps the engine for a stub, or the temp filesystem for an
in-memory fake, proves only that the *wiring* compiles — exactly the layer that
was never in doubt.

**"The thing under test" is named precisely (it is not "every engine in every
test"):** for a file converter the load-bearing thing is the **no-harm / isolation
LAYER** — `crate::fs_guard` (atomic exclusive no-clobber publish on the resolved
real file), `crate::isolation` (the confined subprocess wrapper), and
`crate::outcome` (the error-taxonomy + lossy-disclosure logic). Those are tested
against a **real temp filesystem** and a **real isolated subprocess**, never a
fake. **Engine-*internal* behaviour** (does LibreOffice actually render this DOCX,
does FFmpeg actually remux this MP4) is the domain of the **per-pair integration
level (§2.3 / spec §6.4.3)** with the **real engine** against the **real corpus**;
it is *not* mocked there either. The split is: the safety layer is unit/property-
tested with a real FS + a real subprocess; the engines are integration-tested for
real; nothing that the level is meant to prove is ever stubbed. (spec §6.4.1
"no engines, no real FS *where avoidable*" applies to the *pure-logic* unit layer;
the moment a test claims a conversion, both the engine and the FS are real.)

### 0.2 Output validity = a per-format STRUCTURAL READER decodes the output

A converter can exit `0` and produce garbage: a truncated `moov` atom, a
0-duration track, an all-grey raster, a broken OOXML relationship graph, or the
input bytes passed straight through all "succeed". So the mandatory validity bar
is that a **per-format structural reader actually decodes the produced file** and
finds the expected structure (spec §6.4.3, enforced by **G31**/**G32**):

| Target family | Mandatory structural reader (the validity proof) |
|---|---|
| Audio / video | **`ffprobe`** decodes the output and reports the **expected codec** with **stream count > 0** |
| Raster image | **`vipsheader`** decodes with **nonzero dimensions**, and a "has content" proof (`vips stats` pixel-variance above a threshold — a self-contained non-blank check, **not** PSNR/SSIM against a reference) |
| PDF (text-bearing source) | **`pdftotext`**/poppler **opens** the PDF and returns **nonzero text** (an image-heavy PDF: `pdfimages -list` reports ≥ 1 image) |
| OOXML (DOCX/XLSX/PPTX) | **`unzip`**-able **with a well-formed `[Content_Types].xml`** |
| CSV / TSV | parsed by a **real RFC-4180 reader** (the §3.5.6 native CSV/TSV parser, or the `csv` crate if adopted — spec §6.4.3 / G31) — **not** a bare field-count parity (which passes on mis-quoted / embedded-newline output that is *unparseable*) — **and** the corpus's leading `=`/`+`/`@` cells are asserted **preserved literally as text** (CSV-injection non-execution on the *output* side) |

**Magic re-detect is a pre-screen only, never sufficient.** Re-running the §1.2
magic-byte detector on the output proves only that the file *starts* with the right
header — every garbage case above passes a magic re-detect. So re-detect via §1.2
is an **optional cheap pre-screen**; the structural reader is **REQUIRED** and is
what G31/G32 assert (this ties spec §6.4.3 directly).

**Beyond "it decodes" — the non-trivial / content-bearing sub-assertions (G31).**
A structurally-openable file can still be empty, a passthrough, or a 4-byte stub,
so per pair the integration level additionally asserts:

1. **non-empty + size-plausible** — output size `> 0` and within a sane factor of
   source for the pair (e.g. DOCX→PDF `> 1 KB`; lossless PNG→BMP within ~10× of
   source);
2. **not a passthrough** — where `src_format != tgt_format`, `sha256(output) !=
   sha256(input)`;
3. **content-bearing where text is expected** — text-source→PDF returns a
   **nonzero** `pdftotext` byte count;
4. **document→image is an ACTUAL CONTENT check, never a size floor (G31, r7).** A
   near-white rasterised page (a font-subsetting failure, a headless-display-absent
   blank raster) passes pixel-variance *and* any size floor while being content-
   less, so every document→image fixture carries a required **`expected_text`**
   field in the corpus manifest and the rasterised output is OCR-verified
   (`tesseract <out.png> - --psm 6`, or `pdftotext` where a PDF intermediate
   exists) to **contain** it. (Scope the OCR leg to L4 with a committed
   min-confidence threshold if per-push cost bites — it stays a content check.)

**Break the circular decode for the headline formats (G31).** AVIF/HEIC outputs
are validated a **second time with a DIFFERENT decoder family** — `ffprobe`
(FFmpeg's dav1d/AV1 path for AVIF, HEVC path for HEIC), distinct from the
libvips/libheif that *produced* the file — and **animated WEBP via `ffprobe`**
(not `dwebp`, which cannot decode animated WebP). A producing-library bug that
emits a structurally-valid-but-subtly-wrong file the *same* library tolerates is
caught only by a foreign reader. The "lacks the decoder" skip is decided by
querying the committed **`ffmpeg-allowed-decoders.lock`** golden (G38), **not** a
live `ffmpeg -decoders` call: absent-from-golden → legitimate skip; present-in-
golden-but-absent-from-binary → **G38 hard-fail** (the staged FFmpeg is broken,
not a free pass).

---

## 1. Test levels and what each owns

From cheapest/most-frequent to most-expensive. Each level names its **spec §**
home and its **verifying gate(s)**. The Build-Loop chooses the **highest
technically sensible level** for a change (DoD item (c)); for a *conversion* that
always includes the §0.2 output-validity bar.

| Level | Owns | Tooling | Spec § | Gate(s) |
|---|---|---|---|---|
| **Unit — Rust** | Pure-logic guarantees: output-naming contract, no-clobber/resolved-identity, frozen-set, re-run equivalence, detection table, batch grouping, target/defaults registry, error-taxonomy mapping | `#[cfg(test)]` (`cargo test`) | §6.4.1 | **G15** |
| **Unit — React** | Frontend utils / hooks / components (presentational logic, reducers, formatters) | **Vitest** | §6.4.6 (last line) | **G15** (vitest run) · **G27** (TS coverage floor) |
| **Property / fault-injection** | No-harm + fail-clearly invariants over generated inputs: atomicity-under-interruption, source-byte-identity, divert/fallback, out-of-disk, malformed/adversarial, cancellation, resource budgets | Rust **`proptest`**, TS **`fast-check`** | §6.4.2 | **G16**, **G15**, **G31** |
| **Integration — per-pair** | **The reliability heart.** Every `(source→target)` pair, **real engine → real output file → real temp FS**, validated by the §0.2 structural reader | `cargo test` driving the real engines | §6.4.3 / §6.4.3a | **G31**, **G32**, **G26** |
| **Fuzz** | "Never panic / OOM / UB on arbitrary bytes" on the **DECODE path that runs in-core** (outside the §2.12 boundary) + an unsafe census | **`cargo-fuzz`** (libFuzzer); **`cargo-geiger`** (census) | §6.4.2 (in-core fuzz harness) / §2.12.4 | **G48** (fuzz); `cargo-geiger` informational (**G29**) |
| **E2E** | The real built app driven through the real window: the §5.2 core flow end-to-end | **`tauri-driver`** + **WebdriverIO** (Win/Linux); macOS = defined degraded smoke | §6.4.6 | *no dedicated Gnn* — E2E runs **on** the **G30** platform matrix; the per-push E2E result blocks red `main` via the CI job directly |
| **a11y** | WCAG 2.1 AA: ARIA/role/focus + computed contrast | `vitest-axe` (jsdom) + `@axe-core/webdriverio` (live WebView) | §6.4.6a | **G33a** (per-push), **G33b** (release) |
| **Visual regression** | Screenshot diff of the rendered UI against a baseline | *(no gate — see §9)* | *(none — §6.4.6 family)* | **G34 — VACATED** |

### 1.1 Unit — Rust core (the guarantees layer · §6.4.1 · G15)

Pure-logic tests on the §0.7 modules, **no engines, no real FS where avoidable**.
Each is the §6.4.1 contract made executable:

- **Output naming contract (§2.2):** base-name-kept + target-extension; `(1)`/`(2)`
  numbering before the extension; never hashed / `_converted`; path-limit →
  fail-clearly (no truncation). **Property-tested** with adversarial names (Unicode,
  emoji, RTL, spaces, dots, max-length).
- **No-clobber & resolved identity (§2.1/§2.3):** exclusive-create semantics;
  symlink/junction/hardlink resolution; de-dup of the frozen set by resolved
  identity; refusal to write through a link onto a frozen source.
- **Frozen source set (§2.4):** files appearing after the freeze are never
  ingested; outputs landing in a source folder don't expand the batch (the
  data-structure leg of **T8** — the live-path leg is the **G31** integration
  sub-test, §2.3).
- **Re-run / equivalence (§2.5):** equality on (resolved source + target +
  effective settings); safe fallback to silent numbering when undeterminable.
- **Detection (§1.2):** the magic-byte classification table — every §04 signature
  (JPEG SOI, PNG, RIFF/WEBP, EBML matroska-vs-webm, ISO-BMFF `ftyp` brand, OLE2
  DOC/XLS/PPT disambiguation, ZIP-OPC content-type DOCX/XLSX/PPTX/ODF-mimetype,
  ADTS-vs-MP3, Ogg Vorbis-vs-Opus, ASF WMA-vs-WMV, CSV/TSV delimiter sniff) gets a
  fixture asserting the user-facing type, **including misnamed-extension fixtures**
  (`.jpg` that is PNG; `.m4a` that is ALAC) and the "detected-but-unsupported" /
  "uncertain" outcomes. **Backed by a committed KAT:** a `tests/detect-kat.toml`
  pins each canonical / ambiguous file to its exact `FormatId`, read by the G15
  unit test, so a `quick-xml` bump or a detect refactor that changes an ambiguous
  result is caught at **L2** before the L4 corpus sees it.
- **Batch grouping (§1.3):** one-source-format-per-batch; mixed-drop pre-flight
  refusal lists the found formats; cross-category targets attach to the *source*.
- **Target resolution (§1.5) + defaults registry (§1.6):** for **every** §04
  source format, exactly **one** pre-highlighted default; the "no required choices"
  invariant asserted against the §1.6 registry.
- **Error-taxonomy mapping (§2.8/§2.13):** each failure kind maps to its catalog
  string; the worker-thread panic boundary (`catch_unwind`) surfaces a clean
  per-item failure, not a poisoned pool (the **T5** core-fault leg).

> **No-panic discipline is a compile-time invariant, not a test.** `crate::detection`
> runs untrusted bytes **outside** the §2.12 isolation boundary (security principle
> 9), so a stray `.unwrap()` there is a guaranteed in-core DoS (**T1**). **G4/G14**
> set `clippy::unwrap_used`/`expect_used`/`panic`/`indexing_slicing` to **deny** on
> that path (allow-listed in `#[cfg(test)]` with a `// PANIC:`-justified escape) —
> the lint prevents the class, the **G48** fuzz finds the residue. Tests do not
> have to *find* every panic; they have to *exercise the guards* (§4).

### 1.1a Boot-stage / host-glue (AppHandle-coupled launch glue · source-scan + §1.6 E2E · G28 exemption)

The **launch / host glue** — the §7.8.1 `forward_launch_intake` funnel, the §7.1.1
single-instance callback, the macOS `RunEvent::Opened` handler, the AppHandle
predicate/buffer shells (`converter_is_busy`/`frontend_ready`/`buffer_pending_intake`),
and `fn main()` itself — is **AppHandle-coupled**: it cannot run under `cargo test`
without a Tauri runtime, and **this crate ships no `tauri::test` mock harness BY
DECISION** (a mock runtime would reverse this stance, add a test-feature dependency
surface to the offline supply chain, and still leave the `app.emit`/window arms behind
the runtime). The **boot-stage pattern** tests this glue at the level where it is real:

- **Source-scan signature pins (L1/L2, `#[cfg(test)]`).** The glue's *shape* is pinned by
  fn-pointer coercion (a signature drift fails to compile) — e.g. `launch_intake`'s
  `launch_funnel_items_have_their_spec_signatures` — and its *boot invariants* by
  whole-`main()`-body scans (`production_boot_source` / `production_full_boot_source`:
  no-socket-on-boot, no programmatic window builder, no updater plugin). The dispatch
  *logic* it routes through is the **pure, truth-table-tested** rule extracted beside it
  (`intake_disposition`), so the only thing left un-executed is the AppHandle plumbing.
- **§1.6 E2E real window + §6.6 walkthrough.** The glue's *runtime* behaviour (the emit
  reaching the WebView, the window re-focus, Open-with) is exercised by the real built app
  on the **G30** matrix and the human walkthrough — not by `cargo test`.

**G28 consequence (the boot-glue diff exemption · owner decision A · P2.135).** Because
execution coverage structurally cannot reach AppHandle-coupled glue, the **G28 diff floor
exempts changed lines inside a fn whose signature references an `AppHandle` type** (parameter, return or
bound — all equally runtime-coupled) — else
the gate would fire "for the wrong reason" (the §1.2 no-panic-discipline anchor's sibling:
a gate must not demand a proof the code's blessed test level does not produce). The
exemption is **structural** (`check-coverage` `_apphandle_fn_ranges`, signature-based — not
a marker the Build-Loop can self-apply), **fail-closed** (any parse ambiguity leaves the
line counted), and the exempted lines **stay counted in the G27 per-domain floor** (the
crate's headroom absorbs them; only the change-only diff gate, which a concentrated
boot-glue diff uniquely breaks, exempts them). It is a G28 **scope** refinement, never a
floor relaxation (`diff_floor` stays 80). Pure helpers homed beside the glue
(`intake_disposition`, `parse_path_args` — no `AppHandle` in their signatures) are **not**
exempt: they carry their full §1.1 / §1.3 unit + property bar.

**`fn main()` is NOT reached by the exemption** (it binds no `AppHandle` parameter, so the
signature scan does not match it). That is deliberate and consistent: `main`'s boot body is
covered by the *whole-`main()` source-scans* (`production_full_boot_source` /
`boot_invariants` / `instance_identity`) — the source-scan half of the boot-stage pattern
above — not by the diff exemption. So new uncovered boot logic added directly in `main`'s
`setup` closure is held by those scans, not waved through.

**Residual abuse surface (bounded, documented).** Because the exemption is signature-keyed,
a change could in principle bury genuinely-testable logic inside an `AppHandle`-signature fn
to dodge the diff floor. This is **bounded, not eliminated**, by four independent backstops:
(a) the **G27 per-domain floor still counts every exempted line** — uncovered logic drains
the crate's headroom toward its 70 % floor and eventually reddens G27; (b) the exemption set
is **logged every run** (`[G28] boot-glue exempt: …`), so the diff is auditable; (c) the
**dual review** reads the glue; and (d) the **§0.7 architecture** homes real logic in the
tier modules, not the host glue. The residual is accepted at this bound.

### 1.2 Unit — React (Vitest · §6.4.6 · G15 runs · G27 TS floor)

Frontend utility functions, hooks, and presentational components under **Vitest**
(§0.8). Reducers, formatters, the intake-state machine's pure transitions, and
component render/interaction logic. These do **not** open a WebDriver session — the
live-window behaviour is the E2E level (§1.6). The contrast half of a11y does
**not** run here (jsdom computes no layout, §1.7).

### 1.3 Property / fault-injection (no-harm + fail-clearly · §6.4.2 · G16/G15/G31)

These directly defend the SSOT hard promises and run with a **real (temp)
filesystem** and **real or stub engines** as the case demands (a fault-injection
case that proves "a hanging engine fails one item" needs a real subprocess — the
committed **timeout-sentinel corpus case** (a deterministic input / `#[cfg(test)]`
sidecar that reliably stalls without progress) exercises the §0.9 watchdog parameters
+ the §1.7 reap to `Failed(EngineHang)`; a budget case may use a deterministic stub
harness). Conventions:

- **Shrinking is MANDATORY.** Rust = **`proptest`** (macro-based shrinking, no
  hand-rolled `Shrink` impls). TS = **`fast-check`** (a custom `Arbitrary` must
  delegate to a built-in shrinker — **`fc.gen()` without a shrink wrapper is
  banned**, machine-enforced by **G9 invariant (f)** / a project-local ESLint rule,
  *not* a prose convention).
- **Fixed case counts, determinism engineered.** A **pinned CI seed**, a
  **case-count floor above the thin default 256**, and a property failure is
  **NEVER retried to pass** (§7). A flaky property is a determinism bug to fix, not
  a count to lower.

The §6.4.2 cases this level owns:

- **Atomicity under interruption (§2.1):** a conversion killed mid-write **never**
  leaves a truncated visible file — only a discardable temp artifact, cleaned on
  next run (§2.6). The kill is injected **specifically in the
  post-`sync_all()`-pre-`rename` window** via a `#[cfg(test)]` fence in
  `crate::fs_guard::atomic_publish`, on **all 3 OS**, to exercise the §2.1.3
  two-state invariant at the exact critical boundary. Cross-volume path (source on
  USB → output in Downloads, §2.14) exercises copy→fsync→atomic-rename *within* the
  destination volume. **macOS staged-source-copy case:** kill the app between the
  §3.5.0 staged source copy and the engine spawn, then assert the §2.6.3 startup
  sweep reclaims the staged copy *and* its `run-<RunId>/` dir.
- **No-harm fuzz:** randomized batches assert **source bytes byte-identical
  before/after** every run (the **T2/T7** no-harm proof, also the standing G32(a)
  invariant), including the same-source==same-target re-encode (§2.1) and the
  divert/fallback path (§2.7).
- **Divert / fallback (§2.7):** each concrete unwritable/ephemeral case — (a)
  read-only mount, (b) network share (flips read-only mid-run for the §2.7.2
  late-divert), (c) OS-ephemeral temp dir, (d) cross-volume destination — asserts
  the output lands at the §2.7.3 divert target (or fails clearly when the divert
  target is itself ephemeral), the original is untouched, and the late-divert
  re-checks (link-safety §2.3.3 + path-limit §2.2.3 + per-volume free-space
  §2.14.4) all run.
- **Out-of-disk / too-big (§1.10/§2.8):** a constrained-FS harness proves the item
  fails fast + clearly, the batch continues, free space returns to ~baseline
  (§2.6); a cleanup that itself fails is **never** reported as a clean success.
- **Low-memory / memory-pressure (§1.10 low-memory policy):** a **memory-constrained-host**
  harness (cap available RAM, run a large batch) asserts the app **degrades gracefully** —
  the effective §0.9 degree drops toward serial, the high-memory watermark pauses NEW item
  dispatch (with the §5 `LowMemoryNote`), in-flight items finish, the batch completes, and peak RSS
  stays bounded — **no OOM-crash, no UI freeze**; a single over-budget item is killed to
  `Failed(TooBig)` while the batch continues. `[DEFER: corpus]` for the exact constrained-RAM
  number (calibrated like the §1.10 budgets).
- **Malformed / adversarial inputs (§2.12/§2.13):** truncated, 0-byte,
  fuzzed-header, encrypted/DRM, and decompression-bomb-shaped inputs each produce
  **one plain message**, no crash, no app wedge, batch continues — backed by
  **explicit fixtures** (§5), not only a property concept.
- **Cancellation (§1.7/§1.11):** mid-batch cancel keeps finished items, discards
  the in-flight one with no partial leftover, never touches originals.

The cross-cutting security cases (log-redaction, temp ownership + mode-bits,
T9b/T8/T7/T10 sentinels) are property/integration tests homed in §6 with their
gate refs.

### 1.4 Integration — per-pair conversions (the real engines · §6.4.3 · G31/G32)

The heart of the reliability gate (§6.5). For **every** `(source→target)` pair
across §04 (images/audio/video/documents/spreadsheets/presentations + the
cross-category extract-audio and to-GIF ops), on **each native platform** (§1.6 /
spec §6.4.4), against the §6.4.5 corpus:

- the conversion **completes with exit success** and produces a **valid file of the
  target format** — proven by the §0.2 **structural reader** (G31), reinforced by
  the **non-trivial / content-bearing** sub-assertions and the **cross-library
  decode** for AVIF/HEIC/animated-WEBP;
- **content-fidelity spot-checks:** CJK/RTL text survives doc/sheet/slide
  conversions (§2.10); image orientation baked upright; audio tags/cover-art
  round-trip where supported; video chose the **lossless remux** path when codecs
  already fit;
- **lossy disclosure** fires **iff** the pair is flagged lossy in §04 (and, for
  video, on the *planned* remux-vs-reencode disposition, not the static pair);
- **patent-gapped pairs (§3.4):** on a platform where §3.4 marks a target
  unavailable, the test asserts it is **absent/disabled** (honest unavailability),
  not a failure.

**The pair set is non-circular (§6.4.3a · the bijection guard).**
`scripts/check-corpus-coverage.rs` (a `cargo xtask`-style Rust bin, run in Lane A,
no engines) asserts a **bijection** between the §04 pair matrices and the corpus
`manifest.toml` `covers` lists: every required pair has ≥ 1 backing corpus file,
and every `covers` 2-tuple names a real §04 pair. A pair literally **cannot be
declared `reliable`** without a corpus file whose `covers` names it. (It also
enforces the content-floor tags — `cjk-body`, `rtl-body`, `non-ascii-encoding`,
`non-latin-tags`, `representative-av`, `real-image` — so the corpus is *content*-
complete, not just pair-complete; spec §6.4.5.)

**Mixed-batch state-leak (G31, r7).** The corpus covers single malformed inputs +
T8 self-feeding, but a batch where file N is malformed and N+1 is valid through a
**shared `soffice`-headless process** is only testable as a mixed batch: assert (1)
all valid files produce valid outputs, (2) malformed files fail cleanly, (3) the
valid-file results are **identical to an isolated single-file run** (no cross-file
state leak through the shared LibreOffice process). LibreOffice headless is **not
safely parallel** (§0.9), so office-pair integration runs **serialized** at the
§0.9 concurrency degree the test harness reads from config.

### 1.5 Fuzz — the in-core decode path (§6.4.2 / §2.12.4 · G48)

**Premise:** every third-party decoder runs in an isolated subprocess (§2.12), so a
decoder crash is *contained*. The **one** untrusted-byte path that runs **in the
trust kernel, outside the §2.12 boundary** is the §1.2 detection layer — a panic /
OOM / UB there lands in the core (security principle 9). That surface gets a real
coverage-guided fuzzer; the isolated C/C++ engines do **not** (libFuzzer is
in-process Rust and cannot reach them — their adversarial coverage is the **G26**
fixed corpus fault-injection *through* the boundary, and the reserved **G65**
black-box engine-subprocess fuzz is the build-gates §8 forward item — **G65**).

**The invariant: "never panic / abort / UB on arbitrary bytes."** `cargo-fuzz`
(libFuzzer) targets (**G48**), on the **Linux + macOS nightly** legs (date-pinned
`nightly-YYYY-MM-DD`), with **AddressSanitizer ON** (asserted-not-disabled — a
`--sanitizer none` would silently drop it) + a **UBSan** leg + a small **`cargo
miri`** leg over the pure-logic in-core paths (Miri covers the safe-Rust side; ASAN
covers the FFI boundary):

1. **`crate::detection`/sniff** on a hostile ZIP/OLE2/gzip/svgz/XML corpus — no
   panic/abort; the §1.2 decompression-ratio cap (≤ 100×) and `MAX_SVGZ_SNIFF`
   (≤ 64 KiB) bounds **actually fire**; the XML reader has **DTD/external-entity
   resolution disabled by construction** (`quick-xml`/`roxmltree` with entity
   resolution off — defeats XXE / billion-laughs in the `xl/workbook.xml` / ODS
   `content.xml` peek).
2. **`crate::fs_guard::resolve_identity`** on untrusted PATHS (null bytes,
   overlong UTF-8, max-length, symlink chains, `..`) — no panic, structured
   `Err`, **never `Ok` on a null-byte path** (**T7+T2a**).
3. **`crate::fs_guard::is_safe_output`** (Windows device / reserved-name /
   drive-relative / UNC classes) — no panic, structured `Err`.
4. **the in-core CSV/TSV native engine** (§3.5.6 — memory-safe ≠ panic/OOM-safe:
   gigabyte quoted fields, recursive quoting, NUL bytes) — no panic, bounded output
   relative to input, clear failure beyond a column floor.
5. **archive-entry-name (zip-slip) target** — a crafted OPC/ODF/ZIP whose
   central-directory entry names carry `../`, absolute paths, NUL/overlong-UTF-8:
   the in-core peek never resolves or writes outside its bounded buffer and never
   escapes the per-job scratch root.
6. **`convertia-imgworker`'s own Rust→FFI surface** linked against the staged
   `libvips`/`libheif`/`librsvg`, **ASAN on** — the densest unsafe surface in the
   product (the first Rust touching untrusted image bytes across the C/C++
   boundary). **ASAN-coverage honesty:** the staged engines are pre-compiled
   `.so`/`.dylib`/`.dll` that **cannot** be ASAN-instrumented, so ASAN here catches
   **Rust-side heap violations + FFI-boundary crossings only, NOT bugs inside
   libvips/libheif/librsvg** — a valuable boundary test, *not* a decoder-internals
   fuzz (that is **G65**). A full-internals fuzz would need a from-source
   ASAN build asserted same-version as the `engines.lock` pin (owner-decidable — the
   reserved **G65** full-internals track, build-gates §8).

**The IPC boundary is a `proptest` contract (G16), NOT a libFuzzer target.** It is the
TRUSTED WebView→Rust type door (the app's own bundled, CSP-locked, no-network frontend
feeding structured JSON through **derived safe-Rust `Deserialize`**), so a `proptest`
for robustness is the correct level — malformed input is a `Result::Err` by construction,
not a panic/UB, and there is no unsafe path for coverage-feedback to reach; coverage-guided
libFuzzer stays reserved for the six untrusted-byte surfaces above. Both IPC legs are the
P0.4.3 `IPC_PROPTEST_TARGETS`, in `tests/`, **NOT** under `fuzz/`: **(a)** each
`#[tauri::command]` handler's serde boundary — malformed `serde_json` (null-byte strings,
`MAX_USIZE` arrays, deeply-nested JSON, NaN/Inf) → a structured `Err`, **never a panic**
(the WebView→Rust boundary the **G29** *structural* Semgrep cannot reach — it proves the
shape, not the runtime deserialization); **(b)** **per-numeric-IPC-argument** boundary-value
tests (`u32::MAX`, `i32::MIN`, 0, 1, 2^16−1) → a structured `Err`, not a panic/overflow — a
`MAX_USIZE` integer field is valid JSON that deserializes then can overflow a
`width*height*bpp` preflight (**T10**), so `#![deny(clippy::arithmetic_side_effects)]` on the
IPC-handler module makes unchecked arithmetic a compile warning. *(If a future command ever
ships a hand-written `Deserialize` with non-trivial logic — none exists in v1's derived-serde
IPC — that specific deserializer could merit its own fuzz target.)*

**Bound-firing is STRUCTURALLY proven, not fuzzer-hoped.** libFuzzer "no crash"
cannot distinguish "cap fired" from "cap code never reached", so deterministic
**G16 saved-corpus fixtures** assert each bound: a gzip exactly **101× compressed**
⇒ bounded `Err`, not a full inflate; an svgz exactly **`MAX_SVGZ_SNIFF + 1`** ⇒ the
sniff stops at the limit; a **NUL-byte path** and a **`PATH_MAX`+1 path** ⇒ `Err`
(every platform incl. Windows); a ZIP whose entry is `../../etc/passwd` ⇒ the OPC
peek produces only a *detection result*, reading/writing nothing at the traversal
path. **Windows dangerous-path classes** get ≥ 1 deterministic fixture each: device
paths (`\\.\`, `\\?\`), reserved names (`CON`/`NUL`/`COM1-9`/`LPT1-9` with any
extension — `CON.jpg` opens the console device), drive-relative (`C:foo`), UNC, and
trailing dots/spaces.

**Crash-corpus persistence + cross-platform replay.** Every libFuzzer-found crash
is minimized and committed under tracked `fuzz/corpus/` + `fuzz/crashes/`. The
deterministic replay is a **plain `cargo test` integration test
(`tests/fuzz_replay.rs`)** that feeds every corpus/crash file directly to the
target function with **no libFuzzer harness** — so it compiles and runs on **all
platforms incl. Windows under the stable toolchain**, no nightly/instrumentation
needed. So: **all platforms run the stable-toolchain replay; Linux + macOS
additionally run the instrumented `cargo-fuzz` nightly leg.** A fixed crash cannot
silently regress on any OS, and a **G24 planted-positive** asserts a committed
crash fixture **fails** the replay if its fix is reverted (the replay is armed, not
a no-op). The committed corpus/crash fixtures are integrity-pinned by the **G24a**
fixture manifest (§5).

**Resource bounds are PINNED** (a decompression-bomb / recursive-quote input must
not OOM/hang the shared runner and surface as flaky infra — a denial-of-CI vector):
every fuzz leg pins **`-rss_limit_mb`**, **`-max_len`**, **`-timeout`** (per input),
**`-max_total_time`** (per job), plus the **G56** `timeout-minutes`. A libFuzzer
**OOM or timeout is a FINDING** (minimized + committed to `fuzz/crashes/`),
**never retried**.

**Unsafe census (`cargo-geiger`) is INFORMATIONAL only** — a census of the `unsafe`
surface, *not* an enforcer (version-fragile; never a required green check). The
enforced unsafe policy is **G29**: `#![deny(unsafe_code)]` at every first-party
crate root + a single narrowly allow-listed FFI module.

### 1.6 E2E — the real window (§6.4.6 · runs on the G30 platform matrix)

A headed driver run drives the **built app** through the **real platform WebView**
via **`tauri-driver`** (a WebDriver endpoint), client = **WebdriverIO v9** (the
JS/Node client — chosen because the a11y contrast gate uses `@axe-core/webdriverio`,
a JS-only package a Rust webdriver crate cannot drive). The run exercises the full
§5.2 flow per platform: empty → intake → collected/confirm → target + default →
destination shown → progress → summary → open-folder. The empty/Idle step also
asserts the **"all conversion happens locally, on your machine"** reassurance line
is present (a cheap string-presence check so the offline reassurance can't silently
drop, SSOT *Offline/privacy*).

**Gate relationship (no dedicated E2E `Gnn`).** E2E has **no own gate id**: **G30**
is the cross-platform *build* matrix (native-per-platform build + the macOS universal
`lipo`-both-slices assertion), not an E2E pass/fail criterion. The E2E flow **runs on
that G30 matrix** and its per-push pass/fail **blocks red `main` via the CI job
directly**. A Build-Loop author looking up G30 will find only the build/`lipo`
assertion — the E2E *result* is the CI job's, not a `Gnn` lookup. (Adopting a
dedicated id would need a spec §6.4.6-family contract first, then an id from the
vacant range — the same rule §9 states for the vacated G34.)

**Platform reality (the driver differs · spec §6.4.6):**

- **Windows + Linux** = full `tauri-driver` WebDriver. **Linux runs under `Xvfb`**
  (`xvfb-run -a …`; WebKitGTK will not initialise without an X/Wayland display) and
  points `tauri:options.application` at the **extracted ELF binary** (the AppImage
  is a self-mounting wrapper WebDriver cannot launch) — the binary name is resolved
  **dynamically** from the case-sensitive Tauri `productName`, not hardcoded, then
  `rm -rf squashfs-root/` after. **Windows**: `msedgedriver` is matched to the
  runner's WebView2/Edge build.
- **macOS** = the **defined degraded smoke test** — `tauri-driver` has **no
  WKWebView driver** (Apple's `safaridriver` automates Safari, not an embedded
  WKWebView). CI launches the built app, drives a **synthetic `argv` conversion** of
  one corpus file through the launch-intake path (§7.8/§1.1), and asserts (a) the
  window/process is present, (b) the expected output file appears, (c) exit 0. The
  full WebView UX flow on macOS is the **§6.6 human walkthrough** (which also tests
  Sequoia Gatekeeper + per-sidecar quarantine recovery + TCC prompts).

**What automation cannot synthesise (kept honest):** the OS-level **native
file-drop** (§5.4) is **not** automatable by WebDriver, so the automated E2E uses
the **file-picker path** (C2a `pick_for_intake`, which funnels into the *same* C1
`ingest_paths` as a drop); the native drop is validated in the **§6.6 human
walkthrough**. **macOS TCC** prompts cannot be answered headlessly, so the smoke
leg writes only to a `TMPDIR` (no prompt fires); the TCC exercise is **§6.6**. Plain
**Playwright cannot drive a Tauri WebView** (it is not a CDP target) and is not the
E2E driver here.

### 1.7 a11y — axe (§6.4.6a · G33a per-push / G33b release)

The DoD **basic-a11y** gate (keyboard path + readable contrast/sizes, WCAG 2.1 AA
per §5.6) has an automated half here and a human half (§6.6). **Critical + serious
= 0 blocks** — any axe violation at the configured impact level **fails the build**.

> **Bootstrap (P0 only).** G33a is **fail-open / skip-with-warning in P0** (the
> rendered React tree does not exist yet) and **fail-closed from P1 onward**, once the
> tree exists — the identical bootstrap annotation `G47`/`G27`/`G22`/`G23`/`G57` carry,
> so the P0 green-L4 exit criterion is not blocked by a gate targeting absent code, and
> the gate is never silently fail-open once its target lands.

- **ARIA/role/focus (per-push · G33a):** **`vitest-axe`** (axe-core ^4.4 under
  jsdom, pinned `vitest-axe@0.1.0`) over the rendered React tree — ARIA role/state
  validity (the §5.6 `radiogroup`/`radio` tiles carry valid `aria-checked`),
  focus-order / roving-tabindex sanity, labelled controls. Lane-A, no WebDriver.
  **jsdom limitation: it cannot compute contrast** (no CSS/layout), so the contrast
  rule does **not** run here.
- **WCAG-AA contrast (release-tier · G33b):** **`@axe-core/webdriverio`** against
  the **live WebView** (the §1.6 `tauri-driver` session), `color-contrast` ≥ 4.5:1
  normal / ≥ 3:1 large + UI components, in **both** Light and Dark themes (§5.5),
  on the **Linux + Windows** legs only. **macOS is the acknowledged automated gap**
  (no WKWebView driver) — its contrast check is the **§6.6 human walkthrough**'s
  readable-contrast item, recorded in `docs/usability-floor.md` (an explicit gap,
  not a silent skip).
- **Text size** is not an axe rule: the minimum-body-text-size half (body ≥
  `--text-base` = 16px, §5.5) is the **§6.6 walkthrough**'s job (an optional
  computed-`font-size` belt-and-suspenders assertion MAY ride the
  `@axe-core/webdriverio` session).

The rendered colours come from the §5.5 design tokens; this level is what makes the
§5.6 "WCAG 2.1 AA" claim verifiable rather than aspirational.

### 1.8 Visual regression — see §9

A screenshot-diff level (fixed tolerance, baseline updated only on intentional
change) is **methodologically defined in §9** but has **no release-blocking gate in
v1** — its would-be gate **G34 is VACATED** because a visual-regression gate has no
spec `§`-home. See §9 for the activation rule.

---

## 2. The round-trip invariant — property test AND CI gate (G32)

A naive "A→B→A byte-stable / within tolerance" round-trip is **methodologically
vacuous** for this catalogue: most pairs are **one-way** (HEIC→JPG has no
JPG→HEIC), and where a reverse exists it is usually lossy (XLSX→CSV→XLSX routes the
reverse through LibreOffice) — so a blanket round-trip is either trivially-true
under *any* tolerance or always-false. **G32 replaces it with three sharp,
machine-enumerated invariants:**

1. **(a) SOURCE-UNCHANGED** — `sha256(source_before) == sha256(source_after)` on
   **every** corpus file (the no-harm proof, **T2/T7**). This is also the property
   test of §1.3's no-harm fuzz — the same invariant, asserted both per-corpus-file
   (the CI gate) and over generated batches (the property).
2. **(b) OUTPUT-VALIDITY** — the produced output passes the §0.2 **real structural
   reader** of G31 (not magic-sniff). Everything asymmetric rests on (a)+(b).
   **"Tolerance" is no longer an undefined fudge** — there is no tolerance band,
   only structural validity.
3. **The literal A→B→A byte-stable check is scoped to the small truly-invertible
   lossless set** (e.g. PNG→BMP→PNG, FLAC↔WAV where the pair exists) — and that set
   is **MACHINE-ENUMERATED, not a prose promise**: a committed section in
   `tests/corpus/manifest.toml` lists every byte-stable pair + a one-line
   rationale; **G32 reads it and FAILS** if a listed pair does not round-trip
   exactly, **or** if a pair was added without a rationale. So the Build-Loop cannot
   assert a round-trip for a pair that is *not* truly lossless (e.g. PNG→TIFF→PNG,
   where TIFF strips ICC) with no gate catching the misclassification.

**Plus a pure-logic lossy-disclosure property test** (separate from the
per-corpus-file integration check): a property test in
`crate::outcome`/`crate::format_registry` iterates the **complete `FormatId ×
FormatId` product** asserting `lossy_disclosure(src, tgt) == is_lossy(src, tgt)` —
catching "added a pair but forgot the lossy flag" over the *whole* matrix without a
corpus file per combination (the §6.4.3 per-corpus-file check only proves it for the
files present).

**Plus conversion-output DETERMINISM (c)** — run the same source + same settings
**twice** and assert `sha256(out1) == sha256(out2)`. Non-determinism signals
uninitialized memory, an **embedded-timestamp leak** (a privacy concern for an
offline app), or a randomized path that breaks §2.5 re-run-equivalence. The floor
is **≥ 1 pair per engine PER OUTPUT-FORMAT CATEGORY** (FFmpeg audio/video/container;
LibreOffice word-processing/spreadsheet/presentation; libvips/ImageMagick
per-colour-space — each category enumerated in the corpus manifest so plan-lint
checks the floor is met; "≥ 1 per engine" was too weak — a different muxer/writer
can embed a timestamp the first pair did not). **The floor is met PER PLATFORM:** the
enumerated determinism pairs run on all three native CI legs (§6.4.4) — a category whose
only pair is §3.4-unavailable on a platform is covered by another available pair in that
category on that platform; plan-lint checks the per-platform floor (non-determinism can be
platform-specific — an embedded timestamp / uninitialised padding can appear in one OS'
encoder build and not another). A **`diffoscope`** (the reserved
**G60** tool) **empty-diff** assertion on the double-run pair is the positive proof:
`sha256` inequality says outputs *differ*, `diffoscope` localises it to the embedded
timestamp / PDF XMP `CreateDate` / padding so the leak is **diagnosable**, not just
"differs". For known-non-deterministic encoders (VP9 rate-control, AVIF variable
encode) the exception is **documented in the corpus manifest** and the byte-level
check skipped — but a **non-vacuous-exception guard** asserts every excepted pair
actually produces **different** `sha256` at listing time (if it is in fact
deterministic the exception is **vacuous and must be removed** — the pair became
deterministic after an upgrade, or the exception was masking a real
embedded-timestamp leak that should be **fixed**, not excepted). Run at **L4 on a
schedule**; per-corpus-file cost is one extra conversion.

> **Round-trip is therefore both:** a **property** (the `FormatId × FormatId`
> lossy-disclosure product + the no-harm fuzz over generated batches) **and a CI
> gate** (G32 reading the machine-enumerated lossless-pair list + the per-category
> determinism floor on the L4 schedule).

---

## 3. Corpus & fixtures (§6.4.5 · G24a)

The corpus is a **required v1 asset and a precondition for declaring any pair done**
(SSOT) — without it the reliability gate is circular. The spec owns the **concrete
contents** (§6.4.5: per-format real/synthetic files, the `manifest.toml` shape, the
content-floor tags); this file owns the **conventions** for *how tests reach it*.

- **Single-source helper, auto-discovery.** Tests reach the corpus through **one
  helper** (no inline path duplication, no per-test re-listing). **Drop a file into
  the corpus dir and add its `manifest.toml` entry → a case appears** — the per-pair
  integration level enumerates from the manifest's `covers` lists and the §6.4.3a
  guard, not from a hand-kept test list. There is **ONE root manifest**
  (`tests/corpus/manifest.toml`); `scripts/check-corpus-coverage.rs` reads exactly
  that file, asserts every `[[file]].path` exists on disk, and runs the bijection
  (§1.4). A manifest entry referencing a missing file fails Lane A.
- **Malformed / adversarial / bomb fixtures are FILES, not concepts.** The §6.4.2
  malformed/adversarial cases are backed by **explicit committed fixtures** (§6.4.5):
  truncated / 0-byte / fuzzed-header / encrypted-DRM per format, **and the
  decompression / decompression-bomb set** — an **svgz bomb**, a **ZIP-bomb-in-OPC
  DOCX**, a **deeply-nested PDF flate stream** — so the bomb case is a real input
  fed to the real decoder, not a property abstraction. Plus the **two deterministic
  bound-firing fixtures** (gzip exactly 101× compressed; svgz exactly
  `MAX_SVGZ_SNIFF + 1`) so a removed cap is caught structurally (§1.5, G16/G48), and
  the **adversarial / security sentinel** fixtures of §6 (zip-slip entry names,
  macro-canary office files, `WEBSERVICE()`/external-data, remote-`href` SVG,
  input-side symlinks/junctions).
- **Storage split (§6.4.5).** Small synthetic + CC0 files are **committed in-repo**
  (so the per-push fast lane and the §6.4.3a guard always have them); larger
  real-world media live in an **LFS-backed `corpus-large`** fetched **only** for the
  full Lane-B run — never required for the per-push fast lane. Corpus files **must
  be redistributable** (public-domain / CC0 / self-produced / synthetic); a synthetic
  equivalent that reproduces the same structural property is used where a real
  artifact can't be licensed, and noted in the manifest.
- **Fixture integrity (G24a).** These untrusted-input files are fed to the
  highest-privilege C/C++ decoders, so a poisoned/swapped fixture or a redirected
  LFS pointer must surface as a diff, not a silent substitution. A committed
  **SHA-256 manifest of every tracked corpus/crash fixture** (and the LFS-resolved
  `corpus-large` objects) is verified in CI **before** the corpus runs, plus
  `git lfs fsck` on the Lane-B leg. G24a additionally asserts every corpus path is
  actually `filter=lfs`-tracked per the **effective `git check-attr filter`** (an
  un-tracking `.gitattributes` edit surfaces as a mismatch, not a skip) and that
  `.lfsconfig` (if present) names only the canonical GitHub LFS endpoint;
  `.gitattributes` + `.lfsconfig` join the security-critical-file set. **Update
  protocol:** the SHA-256 manifest is **regenerated by the same `stage-corpus` step**
  that adds a fixture, in the SAME commit; G24a fails on a stale manifest (a
  `git diff --exit-code` of the regenerated manifest), and a plan-lint sub-check
  asserts every `fuzz/corpus/` + `corpus-large/` path has a manifest entry (an added
  fixture with no manifest row fails, not silently passes — same discipline as the
  `engines.lock` SHA rows).

---

## 4. Build fully — no skeleton/stub (wired to the deferral gate · G8)

The owner's core rule (CLAUDE.md §6: the cleanest / most-complete / most-
professional solution **always** wins over token-cost, speed, and "pragmatism")
applies to tests as hard as to code. **A test is not "done" as a skeleton.** The
only sanctioned stub is a **named, compile-time interface shell** filled by a
**named, scheduled** box (the P3 `crate::isolation` shells P4 expands are the
sanctioned example) — never a quiet placeholder, never a "Phase 2 / for now / comes
in P\<n\>" deferral.

This is **machine-enforced by G8** (the deferral / dead-marker gate): a new
production line containing `TODO`/`FIXME`/`unimplemented!`/`todo!`/`unreachable!`/
`dbg!`/`println!`/`console.log`/`": any"`/`as any`/inline `style=`/"stub"/
"placeholder"/"phase 2" — **plus the broadened semantic-deferral vocabulary**
("later"/"for now"/"temporary"/"not yet"/"will add"/"comes in P\<n\>"/"deferred
to"/"once \<phase\>"/"currently absent") — fails unless a **box-id or
`[Build-Session-Entscheidung]`** marker sits within ±6 lines. In production code a
bare `[!extern]` does **not** suppress; only a `[Build-Session-Entscheidung:
box-id]` tag at a documented choice site does.

**Applied to tests specifically:** a `#[test]` body that is `assert!(true)`, a
`todo!()`, a skipped/`#[ignore]`d test without a box-id justification, a Vitest
`it.todo`/`it.skip`, or an integration test that asserts only "the engine returned
no error" (the §0.2 anti-pattern) is a **deferral**, not a test — it fails the box's
DoD item (c) and, where it carries a marker phrase, **G8**. The output-validity bar
(§0.2) is the concrete floor that makes "wrote a real test" checkable: a conversion
box whose test does not read the output back with a structural reader has **not**
tested the conversion.

---

## 5. Coverage — floors, diff gate, shard-merge (§6.4 · G27/G28)

Two **separate** coverage gates, both enforced; neither replaces the other.

### 5.1 Global floor ratchet 50% → 70%, per-domain (G27)

- **Per-domain = per-crate (Rust) / per-package (TS)**, the general floor measured on
  **LINE coverage** by **`cargo-llvm-cov`** (Rust, the LLVM line metric) and **vitest
  v8** (TS). The gate **fails if ANY domain is below its floor — never averaged**
  (averaging lets a well-covered module hide a bare one). The **branch** metric is a
  *separate, additional* floor scoped to exactly three crates (bullet 3 below) — it is
  **not** the general per-domain floor.
- **Ratchet 50% → 70%, increase-only.** The floor lives in a **tracked file**; a
  commit that **lowers** it fails; a raise is a deliberate committed config change
  (no auto-increment). The file is created at **0%** in P0 (no app code yet) and
  **enforces from P1** (annotated `→ activated in P1`) so it does not trip on the
  empty P0 tree.
- **A BRANCH-coverage floor on the three security-critical crates.** Line coverage
  is the wrong metric where the consequence is a missed **branch**: a `fs_guard`
  path-traversal check that rejects `../` but not `..\` has 100% *line* coverage with
  only Unix tests yet leaves the Windows-separator branch untested. So
  **`crate::detection`** (in-core untrusted bytes), **`crate::fs_guard`** (the no-harm
  kernel), and **`crate::isolation`** (the subprocess wrapper) carry a **branch**
  floor via `cargo-llvm-cov --branch`, ratcheting like the line floor — an untested
  platform/error branch in exactly the highest-consequence surfaces is a
  deterministic failure, not a line-coverage false-green. **Sequencing (P1.54):** the
  branch metric needs **nightly** (`-Z coverage-options=branch`) and these three
  modules are **P2/P3** security-kernel code, so the `[branch]` floors land then (with
  the nightly branch run); the P1.54 **line** tier is live now over the foundation
  crates. The `_rust_branch_domain` mapping is already wired.
- **Gate scripts are EXCLUDED from the floors** — they are **G24**-self-tested
  instead (positive + negative). The exclusion glob (`scripts/` + `.github/`) is
  wired into the `cargo-llvm-cov` and vitest v8 invocations from first activation
  (code-enforced, not a prose promise that would otherwise inflate the initial P1
  percentage). The **G48 saved-crash-corpus replay does NOT count** toward the Rust
  floor (a replay is not a coverage source).

### 5.2 Diff-coverage gate ≥ 80% on changed lines (G28)

A **separate** gate: **≥ 80% coverage on changed lines (change-only)**, so new code
**cannot dilute** the global floor while still nudging the average up. The diff gate
is the per-push pressure that keeps each commit's *own* new lines tested; the floor
ratchet is the long-run direction. A change can pass the floor (the tree is already
high) yet fail the diff gate (its new lines are bare) — both must pass.

### 5.3 Single-leg (Lane A) measurement; 3-OS shard-merge is Lane-B `[Build-Session-Entscheidung: P1.54]`

The **per-push G27/G28 coverage is measured on the Linux leg only**: spec §6.7.1 runs
the OS-agnostic step-3 unit/property/fault-injection test leg on the Linux runner (only
compile-sanity fans to the 3-OS matrix), so the per-push floors read a **single Linux
report** — there is no Lane-A shard-merge. (`check-coverage` reads one
`cargo llvm-cov --json`/`--lcov` + one Vitest v8 `json-summary`/`lcov`; the per-domain
roll-up sums covered/count, never averaging per-file percents.)

The **3-OS shard-merge belongs to Lane B** (§6.7.2): the release reliability run
exercises the integration + property + corpus suites on **all three** legs, where
per-OS branch divergence matters. There, to avoid a last-writer-wins race, **each leg
emits a NAMED partial report**, the partials are **merged in a FIXED order**, and the
floor is applied to the **merged report only**. (Per-OS **branch** coverage of the
security kernel — `crate::detection`/`fs_guard`/`isolation` — is the P2/P3 refinement
that lands with those modules + the nightly branch run; see 5.1 bullet 3.)

---

## 6. Cross-cutting security-test homes (defined here · activated by phase)

The security tests that don't belong to one format pair are homed here with their
gate refs; each activates in the phase that introduces its surface. They are mostly
property/integration tests on **G31** / **G15** / **G16**:

- **Log-redaction (§7.5 · G15/G31):** a known secret-looking path stem fed through the
  logger is **absent** from the log (no file contents, no full paths) — a `cargo test`
  (G15 mirror) homed in G31's hosted security-assertion set, parallel to temp-ownership.
- **Temp ownership + mode-bits (§2.14.1 · G15/G31):** `0o700` scratch root / `0o600`
  `.part` publish-temp **+ a Windows ACL leg** — POSIX mode bits are meaningless on
  Windows, so assert the scratch root's DACL grants access **only to the
  current-user SID** (an explicit restrictive DACL at create / `icacls` inspection),
  closing the world-readable-`%TEMP%`-of-decoded-plaintext hole. **+ a
  cleanup-on-fault/kill sub-case** (the per-job kind-2 scratch is removed after a
  forced engine kill mid-conversion) **+ a Windows AV-lock sub-case** — an AV
  scanner can hold scratch files open after process exit, failing `remove_dir_all`
  with `ERROR_SHARING_VIOLATION`; assert cleanup retries-after-release or schedules
  `MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)`, mirroring the §2.1.2 publish-path
  AV-retry.
- **Adversarial-egress / out-of-input read (§0.11 T9b · G42/G42b release; pulled
  forward to the per-push L4 leg on G31):** an adversarial-network corpus — HLS
  `.m3u8` / DASH `.mpd` / `-f concat` script / external-reference-box MP4 (FFmpeg),
  remote-`<img>`/RST-include (pandoc), remote/OLE-link + **`WEBSERVICE()`/external-
  data-range** office files (LibreOffice), remote-`href` + external local-`<image
  href>` `../`-escape SVG (librsvg) — converted **inside the egress-deny window**
  must produce **(a) zero outbound packets AND (b) no out-of-input file read** (a
  known out-of-input **sentinel** the engine must NOT read/embed). The **fs-audit
  half** uses `ptrace` (`docker --cap-add SYS_PTRACE`) or, where unavailable, the
  §2.12.3 **Landlock** tier (`{input ro, scratch rw}`, the grant *is* the
  enforcement) — **Landlock availability is probed before relying on it** (ABI ≥ 1,
  kernel ≥ 5.13), and if **neither** `ptrace` nor Landlock is available the gate
  **FAILS CLOSED** with a diagnosable `::error::` annotation (a mandatory adversarial
  gate that silently no-enforces is worse than a visible red).
- **T9b corpus sentinels (G31):** a `.docm`/`.xlsm`/`.pptm` with an
  `AutoOpen`/`Workbook_Open` macro writing a canary inside the egress-deny window →
  canary **NOT** created (LibreOffice macro-suppression); a `WEBSERVICE()` `.xlsx` →
  no egress + no out-of-input read; **a crafted BMP / an SVG carrying an MSL/URL-
  coder reference → no egress + no out-of-input read** inside the G42/G42b window
  (the ImageMagick coder-class sentinel — the densest-CVE decoder family, §3.5.5);
  a poppler remote-URI-annotation `.pdf` → no packet in the egress-deny window.
- **T8 self-feeding / batch-expansion (live-path leg · G31; G15 is the
  data-structure unit leg):** a batch whose conversion writes outputs **into the
  same watched/dropped source folder mid-run** asserts the fresh outputs are **NOT**
  in the run's ingest/result set (snapshot-not-live-iteration, §2.4.2); a
  **two-instance** fixture where instance B drops a `file`/`*.part` into a shared
  folder mid-run asserts instance A's frozen set never grows (§2.4.3 concurrent-
  instance hand-off).
- **T7 input-side symlink/junction (G31):** a dropped folder containing
  `innocent.jpg -> /etc/passwd` (or a Windows junction → System32) is
  `resolve_identity`-resolved **before** the engine sees it — assert the engine
  receives the **resolved real path**, the frozen set records the resolved identity
  (symlink + target dropped together convert **once**), and a resolved
  non-convertible target fails clearly per §2.8.
- **Windows AV-retry (§2.1.2 · G31):** a fault-injection thread holding a handle on
  the publish target asserts the bounded `MoveFileExW` / `ERROR_ACCESS_DENIED`
  retry fires and recovers or fails to a clean `WriteFailed` — never a panic/silent
  discard.
- **Privilege-drop-tier-applied regression (§2.12.3 · G31):** a positive
  per-platform assertion that the §2.12.3 tier actually **FIRED** (a denied
  syscall/socket/exec is refused inside the engine's own sandbox profile) so the
  silent-by-design degrade can't disable seccomp/Landlock/AppContainer on every run
  unnoticed (the §6.4.2 probe asserts *availability*; this asserts *application*) —
  recording "tier applied vs degraded" per platform in the release evidence. The
  tier **ratchet/trend** itself (G64, the decrease-guarded
  `privilege-drop-coverage.toml`) is the release-tier policy homed in P0.7.
- **§2.12.3 memory-cap kill (G31):** an engine exceeding its Job-Object/`RLIMIT`
  memory budget mid-conversion is killed to a clean `Failed(TooBig|EngineHang)`, the
  batch continues, host RSS returns to baseline.
- **Process-group / Job-Object reap (T10 · G31):** a deliberately-hanging /
  child-spawning sidecar is reaped by the §0.9 Job-Object/process-group kill with no
  orphan/zombie left and the handle count returning to baseline.
- **T10 adversarial resource-budget (§1.10 · G16/G31):** oversized-render SVG,
  over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no
  handle/RAM exhaustion — **plus an output/scratch-BYTE-budget sub-case** (a decoder
  slowly exploding a 1 KB input into a 50 GB intermediate *within* its RAM/time
  budget exhausts the scratch **disk**: a bomb whose decoded output exceeds N× input
  or an absolute scratch ceiling → killed to a clean `Failed(TooBig)`, batch
  continues, scratch returns to baseline; the byte budget lives in the §1.10
  preflight).
- **macOS T11 first-accessor (§3.5.0/§7.2.6 · G31):** the **Rust core PID** (not the
  engine PID) is the first process to access a TCC-protected source path, and the
  engine receives a per-job **kind-2 scratch** path — pairs with the **G29** macOS
  Semgrep `stage_for_tcc`-before-spawn rule.

> **Scoped mutation-testing (owner-decidable, build-gates §8 / homed P0.5):**
> `cargo-mutants`
> over `crate::fs_guard` + `crate::detection` + `crate::outcome` (the
> no-harm/atomicity/no-misroute kernel) as a release-tier informational-then-
> ratcheted gate — *line* coverage proves a line ran, not that a test would CATCH a
> regression there. The informational run outputs a survived-mutant report per
> kernel crate; the ratchet is a tracked `max_survived_mutants.toml` per crate,
> decrease-only; the owner flips informational→required when the count reaches **0**
> for `crate::fs_guard` + `crate::detection`. Its status is tracked in the
> informational→required decision log (gate-status; plan-lint check 23).

---

## 7. Flaky-test policy

Flakiness is treated as a **determinism bug to engineer out**, not a cost to absorb
with retries. The policy is narrow on purpose:

- **Retry ONLY the infra / timeout classes** — a runner DNS hiccup, a transient
  `tauri-driver` connect, a clone/checkout error. A retry is allowed only when the
  failure class is provably non-deterministic *infrastructure*, never a test
  assertion.
- **Auto-retry is E2E-ONLY.** The §1.6 WebDriver / window-launch level — the only
  level with genuine OS-timing nondeterminism (window paint, driver handshake) —
  may auto-retry a bounded number of times. **Unit / property / integration / fuzz
  levels NEVER auto-retry**; a red there is a real finding.
- **A property failure is NEVER retried to pass** (§1.3). The seed is pinned; a
  reproduction is the point.
- **A libFuzzer OOM/timeout is a FINDING, never a retry** (§1.5) — minimized and
  committed to `fuzz/crashes/`.
- **Engineer determinism instead of retrying.** Pin **locale** and **timezone**
  (`TZ`) in the CI environment; turn **animations off**
  (`prefers-reduced-motion`/`animation-duration: 0`) for the §1.6 / §1.7 / §9
  levels; pin the CI **seed** (§1.3); freeze any clock the assertion reads. The
  **conversion-output determinism** invariant (§2 / G32) is the same principle
  applied to the *product*: a non-deterministic output is a bug (an embedded
  timestamp, a privacy leak), not a tolerance to widen.

Determinism is also why the §2 round-trip work bans embedded timestamps via the
`diffoscope` empty-diff check — a flaky *output* and a flaky *test* have the same
root cause and the same fix: remove the nondeterminism, do not retry around it.

---

## 8. Test changes under failure (no green-by-rewrite)

When a box's change turns a **previously-passing** test red, the model's reflex —
**bluntly rewrite / relax / skip / delete the test until it goes green, and assume
that is fine** — is a real and common failure mode: a red test is frequently
**catching a genuine regression in the new code**, and a blind rewrite **hides the
bug** behind a green check. This section adds the discipline that closes that hole.
It **adds verification + visibility, it does NOT remove the ability to change a
test** — changing a test is necessary in the overwhelming majority of cases and
stays a normal, first-class move; it just has to be **verified and recorded**, never
**assumed**.

### 8.1 The rule (code-first default)

- **When a previously-passing test goes red because of a change, the DEFAULT
  assumption is: the CODE is wrong, not the test.** Start there. Read the failure as
  a signal about the new code first, a signal about a stale expectation second.
- **Rewriting / relaxing / skipping / deleting the test to make it pass is
  PERMITTED — and is the right move in the common case — but ONLY after positively
  proving BOTH:**
  1. **the old expectation is genuinely obsolete** — the behaviour *legitimately*
     changed; **cite the spec-`§` / the recorded decision that changed it** (the
     conflict order SSOT > spec > security/process docs > plan > code still governs —
     a behaviour change with no higher-layer source is itself an escalation, not a
     licence to edit the test); **AND**
  2. **the new expectation is correct** — verified **against the spec**, or by
     **reading back the real result** with the §0.2 output-validity bar (a real
     structural reader for a conversion, the real value for a logic test) — **never
     "it's green now, so it's fine"** (green-after-edit is the exact non-proof this
     rule exists to reject).
- **If (1)+(2) cannot both be proven → the code is wrong; fix the code, not the
  test.** The red test did its job.
- A **test-change that flips a test red→green** (a rewritten/relaxed assertion, a
  newly-added suppression marker, a removed/commented-out assertion) is a
  **HIGH-SCRUTINY event for the G1 dual review** (§5 of build-loop.md): the two
  reviewers explicitly ask **"is this suppressing a real regression?"**, and the
  **(1)+(2) justification is recorded in the commit body**.

### 8.2 It does NOT forbid — it flags + requires justification

This is a **flag-and-justify mechanism, not a friction wall**, and **not** a ban.
The premise is that a legitimate, outdated-expectation test change happens in roughly
**95% of cases** and must stay cheap: a justified change passes normally with a
**one-line rationale** (the (1)+(2) citation). The discipline only *fires* on the
**unjustified** rewrite — the silent "make red go green" with no spec/decision cite
and no read-back of the new result. The mechanical signal leg is the **G70
test-suppression-marker gate** (the G8-deferral analogue applied to tests): adding a
test-suppression marker — `#[ignore]`, `it.skip`/`describe.skip`/`.only`,
`test.skip`, an `xfail`, a `#[should_panic]` slapped onto a previously-real
assertion, or a removed/commented-out assertion in a changed test — **fails UNLESS a
justification tag** sits within ±N lines. The tag has **two shapes** — the
changed-expectation form `[Test-Change: <box-id> — old-obsolete+new-correct, §ref]`
(the green-by-rewrite case this rule targets) and, for a marker that is part of a
**brand-new test** with no prior expectation to obsolete (a legitimate net-new
`#[should_panic]` panic-path test, a deliberately-`#[ignore]`d scaffold), the net-new
form `[Test-Change: <box-id> — new-test:<reason>, §ref]`. **G70 FLAGS + REQUIRES the
justification; it does NOT forbid the marker** — either tag shape passes. The **semantic** question ("is this rewrite
actually legitimate, or is it hiding a regression?") stays the **G1 dual review's**
job (§8.1 last bullet); G70 is the cheap mechanical signal that *surfaces* the change
so the dual review and the commit reader cannot miss it.

### 8.3 Why this is not a new DoD item

The doctrine here is **already inside** DoD item (c) ("tests at the highest
technically sensible level are green" + the output-validity bar) and item (b)
("spec/docs synced in the same commit" — the spec-`§`/decision that obsoleted the old
expectation). This section makes the **handling under failure** explicit and gives it
a mechanical signal (G70) + a dual-review scope, exactly as the
no-skeleton rule (§4) is enforced by G8 without being a DoD item. The **8-point DoD
is unchanged** — this is an anti-pattern + a gate + a dual-review scope, not a ninth
item.

---

## 9. Visual regression (methodology defined; gate VACATED in v1)

The visual-regression *method* is defined so that adopting it later needs no new
design call — only a spec `§`-home and an id:

- **Mechanism:** a screenshot of a rendered UI surface (the §1.6 `tauri-driver`
  session on Windows/Linux, where real layout exists) diffed against a committed
  **baseline** image, with a **fixed pixel tolerance** (a small, committed
  threshold — not an open "looks close enough" band, the same anti-fudge stance as
  the §2 "no tolerance band, only structural validity" rule).
- **Baseline update only on INTENTIONAL change.** A baseline image is updated
  **only** in a commit that *deliberately* changes the UI, with the change called
  out in the commit body (a baseline bump is a reviewable diff, never an automatic
  "accept new screenshot"). An accidental visual drift therefore fails; a deliberate
  redesign updates the baseline in the same commit that causes it.
- **Determinism prerequisites:** the §7 pins (locale, `TZ`, animations off, fixed
  window size, fonts from the bundled set only) — a screenshot diff is worthless
  without them.

**Why no gate in v1 (G34 — VACATED).** A release-blocking gate must have a spec
`§`-home; §6.4.6 is the WebDriver flow and §6.4.6a is axe — **neither owns
visual-regression**, so the id **G34 stays reserved/unused** rather than adding a
blocking gate with no spec contract. **To adopt it:** add a spec §6.4.6-family
entry first, then assign an id from the vacant range above G56 (G34 does not
renumber, so existing references stay stable). Until then, visual regression is an
**optional, non-blocking** developer aid run on the same `tauri-driver` session,
not a DoD bar.

---

## 10. How a box picks its level (the per-box decision)

At **step 4 of the loop** ([build-loop.md](build-loop.md) §3), the Build-Loop picks
the **highest technically sensible level** (DoD item (c)). The decision is
mechanical:

1. **Pure logic** (naming, detection, defaults, error mapping, a reducer) →
   **unit** (§1.1 / §1.2), `proptest`/`fast-check` where an invariant holds over a
   range of inputs.
2. **A no-harm / fail-clearly invariant** (atomicity, divert, out-of-disk,
   cancellation, source-byte-identity) → **property / fault-injection** (§1.3) with
   a **real temp FS**.
3. **A conversion** (any `(source→target)` pair) → **per-pair integration** (§1.4)
   with the **real engine** and the §0.2 **output-validity** structural reader —
   never "the engine returned no error". Add the corpus file + `manifest.toml`
   entry (the §6.4.3a guard then enumerates it automatically).
4. **An in-core untrusted-byte surface** (`detect`, `fs_guard`, the in-core
   CSV/TSV engine, the imgworker FFI shim) → add/extend a **`cargo-fuzz`** target
   (§1.5) **and** a deterministic bound-firing fixture. *(The `#[tauri::command]`
   serde boundary is NOT an untrusted-byte surface — it is the trusted WebView→Rust
   door — so it takes a **G16 `proptest`** in `tests/`, never a `cargo-fuzz` target;
   §1.5.)*
5. **A UI flow** → the **E2E** §5.2 walkthrough (§1.6) on the platforms the driver
   supports; **a component/hook** → **Vitest** (§1.2); **anything rendered** → the
   **axe** a11y level (§1.7).
6. **A new engine staged** → the engine's per-pair corpus + the **G26** non-
   adversarial output-validity leg + the **G32** determinism floor for its
   output-format categories, and re-run the §6.5 reliability gate before it can ship
   (§6.5.4).

**If a chosen test box's prerequisite is unbuilt, build it first — never leave a
hole.** A test box often needs an unbuilt support box: the single-source corpus
helper (§3), a `manifest.toml` entry, a `tests/detect-kat.toml` row, a fixture. When
the chosen box carries a **`needs: P<x>.<y>`** pointing at an unbuilt box, the loop
**builds that prerequisite first** (recursively, following its own `needs:`), then
**RETURNS** to the original box — per [build-loop.md](build-loop.md) DECISION C
(dependency-following, **not** box-skipping). A `needs:` on a *buildable* box is a
dependency to satisfy in place, never a `[!]`/`[!extern]` skip-and-report.

If the spec is ambiguous about *what* the level must assert → **escalate** (§7 of
build-loop.md), do not improvise a weaker test.

---

## 11. References

- Technical home (the pipeline + corpus + reliability ledger):
  [spec §6.4](../spec/06-build-test-release.md) (`§6.4.1`–`§6.4.6a`), §6.5, §6.6.
- Enforcement (the gates named above): [build-gates.md](../security/build-gates.md)
  — **G8** (deferral), **G15** (unit), **G16** (bound-firing/resource fixtures),
  **G24/G24a** (gate + fixture self-test/integrity), **G26** (engine-side corpus),
  **G27/G28** (coverage floor + diff gate), **G29** (SAST/unsafe policy;
  `cargo-geiger` informational), **G30** (cross-platform build matrix the E2E flow
  runs on), **G31/G32** (per-pair corpus + structural readers + round-trip/
  determinism), **G33a/G33b** (a11y), **G34** (vacated visual-regression),
  **G42/G42b** (egress / fs-audit), **G48** (in-core fuzz), **G60** (`diffoscope`),
  **G70** (test-suppression-marker gate — the §8 no-green-by-rewrite signal leg; the
  semantic "is this rewrite legitimate" check stays the G1 dual review).
- Threat model the security tests defend:
  [security-concept.md](../security/security-concept.md) §4–§5 (T1, T2, T5, T7, T8,
  T9b, T10, T11).
- The loop that applies this doctrine: [build-loop.md](build-loop.md) §3 step 4,
  §5 (DoD item (c)).
- Plan home: [docs/plan/P0-build-and-security.md](../plan/P0-build-and-security.md)
  §P0.5.
- Owner's core rule (completeness > pragmatism): [CLAUDE.md](../../CLAUDE.md) §6.
