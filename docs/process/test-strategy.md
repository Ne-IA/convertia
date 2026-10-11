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
level (§1.4 / spec §6.4.3)** with the **real engine** against the **real corpus**;
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
| CSV / TSV | parsed by a **real RFC-4180 reader** (the in-workspace `csv` crate, §3.5.6 — spec §6.4.3 / G31) — **not** a bare field-count parity (which passes on mis-quoted / embedded-newline output that is *unparseable*) — **and** the corpus's leading `=`/`+`/`@` cells are asserted **preserved literally as text** (CSV-injection non-execution on the *output* side) |

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
   **No v1 subject:** no §04 matrix offers a document- or
   presentation-to-image pair (P4.59), so this rule binds once one is added.

**Break the circular decode for the headline formats (G31).** AVIF/HEIC outputs
are validated a **second time with a DIFFERENT decoder family** — `ffprobe`
(FFmpeg's dav1d/AV1 path for AVIF, HEVC path for HEIC), distinct from the
libvips/libheif that *produced* the file — and **animated WEBP via `ffprobe`**
(not `dwebp`, which cannot decode animated WebP). A producing-library bug that
emits a structurally-valid-but-subtly-wrong file the *same* library tolerates is
caught only by a foreign reader. FFmpeg applies an HEIC's container crop aligned
to 4:2:0 chroma, so an odd width or height reads back 1 px short whichever HEVC
encoder wrote the file (measured: 764×508 for 765×509); the HEIC leg reads with
`-apply_cropping none`, crops after the RGB conversion and takes alpha from the
auxiliary stream. The "lacks the decoder" skip is decided by
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
| **Fuzz** | "Never panic / OOM / UB on arbitrary bytes" on the **DECODE path that runs in-core** (outside the §2.12 boundary) | **`cargo-fuzz`** (libFuzzer) | §6.4.2 (in-core fuzz harness) / §2.12.4 | **G48** |
| **E2E** | The real built app driven through the real window: the §5.2 core flow end-to-end | **`tauri-driver`** + **WebdriverIO** (Win/Linux); macOS = defined degraded smoke | §6.4.6 | *no dedicated Gnn* — E2E runs **on** the **G30** platform matrix; the per-push E2E result blocks red `main` via the CI job directly |
| **a11y** | WCAG 2.1 AA: ARIA/role/focus + computed contrast | `vitest-axe` (jsdom) + `@axe-core/webdriverio` (live WebView) | §6.4.6a | **G33a** (per-push), **G33b** (release) |
| **Visual regression** | Screenshot diff of the rendered UI against a baseline | *(no gate — see §9)* | *(none — §6.4.6 family)* | **G34 — VACATED** |

### 1.1 Unit — Rust core (the guarantees layer · §6.4.1 · G15)

**Owns:** the spec §6.4.1 pure-logic contracts on the §0.7 modules, **no engines, no
real FS where avoidable** — output naming (§2.2), no-clobber and resolved identity
(§2.1/§2.3), the frozen source set (§2.4), re-run equivalence (§2.5), the detection
table with its misnamed-extension fixtures (§1.2), batch grouping (§1.3), target
resolution and the defaults registry (§1.5/§1.6), and the error-taxonomy mapping with
the `catch_unwind` panic boundary (§2.8/§2.13; the **T5** core-fault leg) → spec
§6.4.1 · G15.

- **Detection is backed by a committed KAT:** a `tests/detect-kat.toml` pins each
  canonical / ambiguous file to its exact `FormatId`, read by the G15 unit test, so a
  `quick-xml` bump or a detect refactor that changes an ambiguous result is caught at
  **L2** before the L4 corpus sees it.
- **The frozen-set unit test is the data-structure leg of T8;** the live-path leg is
  the **G31** integration sub-test (§6).

> **No-panic discipline is a compile-time invariant, not a test.** `crate::detection`
> runs untrusted bytes **outside** the §2.12 isolation boundary (security principle
> 9), so a stray `.unwrap()` there is a guaranteed in-core DoS (**T1**). **G4/G14**
> set `clippy::unwrap_used`/`expect_used`/`panic`/`indexing_slicing` to **deny** on
> that path (allow-listed in `#[cfg(test)]` with a `// PANIC:`-justified escape) —
> the lint prevents the class, the **G48** fuzz finds the residue. Tests do not
> have to *find* every panic; they have to *exercise the guards* (§1.5, the
> bound-firing fixtures).

### 1.1a Boot-stage / host-glue (AppHandle-coupled launch glue · source-scan + §1.6 E2E · G28 exemption)

The **launch / host glue** — the §7.8.1 `forward_launch_intake` funnel, the §7.1.1
single-instance callback, the macOS `RunEvent::Opened` handler, the AppHandle
predicate/buffer shells (`converter_is_busy`/`frontend_ready`/`stash_pending_intake` —
the §7.8.1-named stash, renamed from the P2.54-era `buffer_pending_intake` at P3.77),
and `fn main()` itself — is **AppHandle-coupled**: it cannot run under `cargo test`
without a Tauri runtime, and **this crate ships no `tauri::test` mock harness BY
DECISION** (a mock runtime would reverse this stance, add a test-feature dependency
surface to the offline supply chain, and still leave the `app.emit`/window arms behind
the runtime). The **boot-stage pattern** tests this glue at the level where it is real:

- **Source-scan signature pins (L1/L2, `#[cfg(test)]`).** The glue's *shape* is pinned by
  fn-pointer coercion (a signature drift fails to compile) — e.g. `launch_intake`'s
  `launch_funnel_items_have_their_spec_signatures` — and its *boot invariants* by
  whole-`main()`-body scans (`production_boot_source` / `all_production_source`:
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
covered by the *whole-`main()` source-scans* (`all_production_source` /
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

**Owns:** the spec §6.4.2 cases — atomicity under interruption (the kill lands in the
post-`sync_all()`-pre-`rename` window on all 3 OS, G15), the no-harm fuzz (the **T2/T7**
source-unchanged invariant of G32 (a) over generated batches), the four §2.7
divert/fallback scenarios, out-of-disk / too-big, malformed / adversarial inputs
(backed by the §3 fixture files) and cancellation → spec §6.4.2 · G15/G16/G31/G32(a).
The memory-pressure case is this file's own:

- **Low-memory / memory-pressure (§1.10 low-memory policy):** a **memory-constrained-host**
  harness (cap available RAM, run a large batch) asserts the app **degrades gracefully** —
  the effective §0.9 degree drops toward serial, the high-memory watermark pauses NEW item
  dispatch (with the §5 `LowMemoryNote`), in-flight items finish, the batch completes, and peak RSS
  stays bounded — **no OOM-crash, no UI freeze**; a single over-budget item is killed to
  `Failed(TooBig)` while the batch continues. `[DEFER: corpus]` for the exact constrained-RAM
  number (calibrated like the §1.10 budgets).

The cross-cutting security cases (log-redaction, temp ownership + mode-bits,
T9b/T8/T7/T10 sentinels) are property/integration tests homed in §6 with their
gate refs.

### 1.4 Integration — per-pair conversions (the real engines · §6.4.3 · G31/G32)

The heart of the reliability gate (§6.5). **Owns:** every `(source→target)` pair across
§04 (+ the cross-category extract-audio and to-GIF ops) on each native platform
(§6.4.4) against the §6.4.5 corpus — the §0.2 structural reader with its non-trivial
sub-assertions and cross-library decode, the content-fidelity spot-checks, lossy
disclosure **iff** the pair is flagged lossy, honest unavailability for the §3.4
patent-gapped pairs, and the non-circular pair set (the §6.4.3a corpus↔pair bijection
guard `scripts/check-corpus-coverage.rs` with the §6.4.5 content-floor tags) → spec
§6.4.3 / §6.4.3a · G31/G32/G26.

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
fixed corpus fault-injection *through* the boundary, and the reserved **G65** weekly
scheduled black-box engine-subprocess fuzz (gate-status.md; P9.36)).

**The invariant: "never panic / abort / UB on arbitrary bytes."** **Owns:** the six
`cargo-fuzz` (libFuzzer) targets — `crate::detection`/sniff, `fs_guard::resolve_identity`,
`fs_guard::is_safe_output`, the in-core CSV/TSV engine, the archive-entry-name (zip-slip)
target and `convertia-imgworker`'s Rust→FFI surface (ASAN sees the Rust side and the FFI
crossings, never the decoder internals) — with their per-target invariants, the
date-pinned Linux + macOS nightly legs (AddressSanitizer on Linux; sanitizer-less
coverage-guided on macOS behind a per-run ASAN re-arm canary), the pinned libFuzzer
resource bounds and the deterministic **G16** bound-firing fixtures → the G48 row · spec
§6.4.2 (the in-core detector fuzz harness) / §2.12.4 · the gate-status G48 entry. Miri
and UBSan legs are not part of v1 (declined, gate-status.md). The five Windows
dangerous-path classes seed `resolve_identity` (§2.3.1's untrusted-path fn), never
`is_safe_output`, whose §2.3.3 contract is the pure no-clobber verdict (`Ok(Safe)` against
an empty frozen set is correct); its invariant is no panic and **never `Ok(Safe)` on an
unresolvable or NUL target** (the fallback is gated on exactly
`NotFound`/`NotADirectory`; own bound-firing seed `nul_output_path`).

**The IPC boundary is a `proptest` contract (G16), NOT a libFuzzer target.** It is the
TRUSTED WebView→Rust type door (the app's own bundled, CSP-locked, no-network frontend
feeding structured JSON through **derived safe-Rust `Deserialize`**), so a `proptest`
for robustness is the correct level — malformed input is a `Result::Err` by construction,
not a panic/UB, and there is no unsafe path for coverage-feedback to reach; coverage-guided
libFuzzer stays reserved for the six untrusted-byte surfaces above. Both IPC legs are the
P0.4.3 `IPC_PROPTEST_TARGETS`, homed in the `#[cfg(test)]` module `ipc_boundary_proptest` of
`src-tauri/src/ipc/mod.rs`, **NOT** under `fuzz/`: **(a)** each
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

**Crash-corpus persistence + cross-platform replay.** Every libFuzzer-found crash
is minimized and committed under tracked `fuzz/corpus/` + `fuzz/crashes/`, and a
**plain `cargo test` replay suite — the crate-root `#[cfg(test)]` module
`crate::fuzz_replay`** (`src-tauri/src/fuzz_replay.rs`; in-crate because the minimal-pub
library keeps `crate::detection`/`fs_guard` out of a `tests/`-dir target's reach) feeds
every file to its target function with **no libFuzzer harness**: all platforms run that
stable-toolchain replay, Linux + macOS add the instrumented nightly leg, and a **G24
planted-positive** asserts a committed crash fixture **fails** the replay if its fix is
reverted. The committed corpus/crash files are plain git-tracked files (never LFS), pinned
by git content addressing; `check-fuzz-contract` requires each named bound-firing seed (§3).

**Resource bounds are PINNED** on every fuzz leg (the G48 row). A libFuzzer
**OOM or timeout is a FINDING** (minimized + committed to `fuzz/crashes/`),
**never retried**.

**No `unsafe` census runs** (`cargo-geiger` is declined for v1, gate-status.md). The
enforced unsafe policy is **G29**: `#![deny(unsafe_code)]` at every first-party
crate root + a single narrowly allow-listed FFI module.

### 1.6 E2E — the real window (§6.4.6 · runs on the G30 platform matrix)

**Owns:** the **built app** driven through the **real platform WebView** — the full §5.2
flow per platform with the Idle offline-reassurance line asserted: `tauri-driver` +
WebdriverIO v9 on Windows and Linux (Linux under `Xvfb` against the extracted ELF
binary, Windows with the runner-matched `msedgedriver`), the defined degraded macOS
smoke (a synthetic `argv` conversion through the launch-intake path), the file-picker
path standing in for the non-automatable native drop, and the §6.6 human walkthrough
for what automation cannot synthesise (the native drop, the macOS WebView flow,
Gatekeeper/quarantine recovery, TCC prompts) → spec §6.4.6 (with §6.4.4).

**Gate relationship (no dedicated E2E `Gnn`).** E2E has **no own gate id**: **G30**
is the cross-platform *build* matrix (native-per-platform build + the macOS universal
`lipo`-both-slices assertion), not an E2E pass/fail criterion. The E2E flow **runs on
that G30 matrix** and its per-push pass/fail **blocks red `main` via the CI job
directly**. A Build-Loop author looking up G30 will find only the build/`lipo`
assertion — the E2E *result* is the CI job's, not a `Gnn` lookup. (Adopting a
dedicated id would need a spec §6.4.6-family contract first, then an id from the
vacant range — the same rule §9 states for the vacated G34.)

### 1.7 a11y — axe (§6.4.6a · G33a per-push / G33b release)

The DoD **basic-a11y** gate (keyboard path + readable contrast/sizes, WCAG 2.1 AA
per §5.6) has an automated half here and a human half (§6.6). **Critical + serious
= 0 blocks** — any axe violation at the configured impact level **fails the build**.

**Owns:** ARIA role/state validity, focus order and labelled controls under `vitest-axe`
on jsdom, per push (G33a); WCAG-AA `color-contrast` in both themes via
`@axe-core/webdriverio` on the live Linux + Windows WebView, release-tier (G33b); the
text-size half is the §6.6 walkthrough's → spec §6.4.6a · G33a/G33b. **macOS is the
acknowledged automated gap** for contrast (no WKWebView driver) — its contrast check is
the **§6.6 human walkthrough**'s readable-contrast item, recorded in
`docs/usability-floor.md` (an explicit gap, not a silent skip).

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
under *any* tolerance or always-false. **G32 replaces it with sharp,
machine-enumerated invariants** — there is no tolerance band, only structural validity.

**Owns:** (a) source-unchanged on every corpus file (the **T2/T7** no-harm proof) and (b)
output validity by the §0.2 structural reader; the literal A→B→A check only over the
lossless-pair section of `tests/corpus/manifest.toml` (a listed pair that does not
round-trip exactly, or one added without its rationale, fails); the pure-logic
`FormatId × FormatId` lossy-disclosure property in the §3.2 registry module; and (c)
conversion-output determinism — the same source and settings twice give the same
`sha256`, ≥ 1 pair per engine per output-format category on all three native legs, run
by the per-engine determinism boxes the G32 row names, with the `diffoscope` (**G60**)
empty-diff proof and the non-vacuous-exception guard over the documented
non-deterministic encoders, at L4 on a schedule → the G32 row · spec §6.4.4.

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
- **Owns:** the fixture files and their integrity — the malformed / adversarial /
  decompression-bomb fixtures as committed files (truncated / 0-byte / fuzzed-header /
  encrypted-DRM per format; the two bound-firing fixtures are §1.5's, the security
  sentinels §6's), the storage split (small synthetic + CC0 files in-repo, real-world
  media in the LFS-backed `corpus-large` for Lane B only, every file redistributable or
  a noted synthetic equivalent) and the committed SHA-256 fixture manifest with its
  LFS-governance legs → spec §6.4.2 / §6.4.5 · G24a.
- **Update protocol:** the SHA-256 manifest is **regenerated by the same
  `stage-corpus` step** that adds a fixture, in the SAME commit; G24a fails on a stale
  manifest (a `git diff --exit-code` of the regenerated manifest); G24a's bijection leg
  fails an added corpus fixture with no manifest row. The in-repo `fuzz/corpus/` +
  `fuzz/crashes/` seeds carry no manifest (git content addressing, §1.5).

---

## 4. Build fully — no skeleton/stub (wired to the deferral gate · G8)

The owner's core rule (CLAUDE.md §6: the cleanest, most complete, most professional
solution for the work at hand **always** wins over token cost, session speed and
pragmatism) applies to tests as hard as to code. **A test is not "done" as a skeleton.** The
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

## 5. Coverage — floors and diff gate (§6.4 · G27/G28)

Two **separate** coverage gates, both enforced; neither replaces the other.

### 5.1 Global floor ratchet 50% → 70%, per-domain (G27)

**Owns:** the per-domain **LINE** floor (per crate / per package, `cargo-llvm-cov` +
vitest v8; any domain below its floor fails — **never averaged**, so a well-covered
module cannot hide a bare one), its increase-only 50% → 70% ratchet in the tracked
`coverage-floors.toml`, the separate **BRANCH** floor on `crate::detection` /
`fs_guard` / `isolation` (seeded by the nightly branch run; the line tier is live per
push), the code-enforced exclusion of the gate scripts (`scripts/` + `.github/`,
**G24**-self-tested instead) and the **G48** crash-corpus replay counting toward no
floor → the G27 row.

### 5.2 Diff-coverage gate ≥ 80% on changed lines (G28)

A **separate** gate: **≥ 80% coverage on changed lines (change-only)**, so new code
**cannot dilute** the global floor while still nudging the average up. The diff gate
is the per-push pressure that keeps each commit's *own* new lines tested; the floor
ratchet is the long-run direction. A change can pass the floor (the tree is already
high) yet fail the diff gate (its new lines are bare) — both must pass.

### 5.3 Single-leg (Linux) measurement

The **per-push G27/G28 coverage is measured on the Linux leg only**: spec §6.7.1 runs
the OS-agnostic step-3 unit/property/fault-injection test leg on the Linux runner (only
compile-sanity fans to the 3-OS matrix), so `check-coverage` reads **one report** (one
`cargo llvm-cov --json`/`--lcov` + one Vitest v8 `json-summary`/`lcov`), the per-domain
roll-up sums covered/count and never averages per-file percents, and there is **no
shard-merge**. Per-OS **branch** coverage of the security kernel
(`crate::detection`/`fs_guard`/`isolation`) is the nightly branch run (§5.1).

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
  Windows, so assert the created scratch root's **effective** DACL grants **no
  broad principal** (Everyone / `BUILTIN\Users` / Authenticated Users) an ACE, via
  a SID-level security-descriptor/`icacls` query (never a localized principal-name
  string match). The delivering mechanism is §2.14.1's `[DECIDED]` **inherited
  default per-user profile ACL** ("no explicit broadening" — the machine-trust
  SYSTEM/Administrators principals retain their inherited ACEs, just as `0o700`
  never excludes root on POSIX), closing the
  world-readable-`%TEMP%`-of-decoded-plaintext hole. **+ a
  cleanup-on-fault/kill sub-case** (the per-job kind-2 scratch is removed after a
  forced engine kill mid-conversion) **+ a Windows AV-lock sub-case** — an AV
  scanner can hold scratch files open after process exit, failing `remove_dir_all`
  with `ERROR_SHARING_VIOLATION`; assert the §2.6.4-`[DECIDED]` model: cleanup
  makes its **single bounded attempt** and surfaces the held path as
  `CleanupResidue` (never a silent leak, never an in-process retry spin), and the
  §2.6.3 sweep / opportunistic reclaim removes it after the handle releases
  — the publish-path §2.1.2 in-process AV-retry is deliberately NOT mirrored on
  the cleanup path (reclaim has a decided asynchronous home).
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
  the publish target asserts the bounded §2.1.2 Windows publish retry on a
  sharing-violation or access-denied status fires and recovers or fails to a clean
  `WriteFailed` — never a panic/silent discard.
- **Privilege-drop-tier-applied regression (§2.12.3 · G31):** a positive
  per-platform assertion that the §2.12.3 realized tier actually **FIRED** (a denied
  syscall, socket or out-of-sandbox access is refused inside the engine's own
  confinement) so the silent-by-design degrade can't disable the §2.12.3 realized tier
  on every run unnoticed (the §6.4.2 probe asserts *availability*; this asserts
  *application*) —
  recording "tier applied vs degraded" per platform in the release evidence. The
  tier **ratchet/trend** itself (G64, the decrease-guarded
  `privilege-drop-coverage.toml`) is the release-tier policy homed in P0.7.
- **§1.10 per-item memory-ceiling kill (G31):** an engine exceeding the §1.10 per-item
  memory ceiling mid-conversion is killed to a clean `Failed(TooBig)` on each OS its §1.10
  per-OS row realizes, the batch continues, host RSS returns to baseline.
- **Process-group / Job-Object reap (T10 · G31):** a deliberately-hanging /
  child-spawning sidecar is reaped by the §0.9 Job-Object/process-group kill with no
  orphan/zombie left and the handle count returning to baseline.
- **T10 adversarial resource-budget (§1.10 · G16/G31):** oversized-render SVG,
  over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no
  handle/RAM exhaustion — **plus an output/scratch-BYTE-budget sub-case** (a decoder
  slowly exploding a 1 KB input into a 50 GB intermediate *within* its RAM/time
  budget exhausts the scratch **disk**: a bomb whose decoded output exceeds N× input
  or an absolute scratch ceiling → killed to a clean `Failed(TooBig)`, batch
  continues, scratch returns to baseline; the byte budget is §1.10 point 3).
- **macOS T11 first-accessor (§3.5.0/§7.2.6 · G31):** the **Rust core PID** (not the
  engine PID) is the first process to access a TCC-protected source path, and the
  engine receives a per-job **kind-2 scratch** path — pairs with the **G29** macOS
  Semgrep `stage_for_tcc`-before-spawn rule (per the 2026-08-30 P4.26 adjudication the
  paths-scoped door tripwire with deliberately zero in-scope spawn sites — staging
  sits upstream at the §3.2.2 `engine_input` seam) plus the non-vacuous check-sast
  `t11_seam_pin` on that seam's macOS arm; the behavioural burden sits on this G31
  leg.

> **Scoped mutation testing (the G15 sub-leg).** `cargo-mutants` over
> `crate::fs_guard` + `crate::detection` + `crate::outcome` — line coverage proves a
> line ran, not that a test would CATCH a regression there. It re-runs at every
> phase-end sweep (§11.2) against the decrease-only per-module
> `max_survived_mutants.toml` ratchet; its informational→required posture is the
> gate-status ledger's (plan-lint check 23).

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
- **Engineer determinism instead of retrying.** A test injects the clock, timezone
  and locale it reads instead of taking the host's (a paused or fake clock for a
  timing rule, a fixed timezone and locale), and every engine subprocess runs in a
  minimal environment with a pinned `LC_ALL` (spec §3.5 shared invocation
  conventions, §2.12.3), so conversion output never depends on the runner's locale. A
  real subprocess runs on the host's clock, so a test that waits on one arms its
  assertion off an event it observes and bounds the wait generously, never on a
  wall-clock guess. Turn **animations off**
  (`prefers-reduced-motion`/`animation-duration: 0`) for the §1.6 / §1.7 / §9
  levels; pin the property-test **seed** (§1.3). The
  **conversion-output determinism** invariant (§2 / G32) is the same principle
  applied to the *product*: a non-deterministic output is a bug (an embedded
  timestamp, a privacy leak), not a tolerance to widen.

Determinism is also why the §2 round-trip work bans embedded timestamps via the
`diffoscope` empty-diff check — a flaky *output* and a flaky *test* have the same
root cause and the same fix: remove the nondeterminism, do not retry around it.

### 7.1 Observed flakes become boxes

An observed non-infra flake on a never-auto-retry level (unit / property /
integration / fuzz) is a determinism bug (§7). The observing session records it as a
`[TEST]` box in the phase that owns the flaky test (the current phase when that one is
closed) — appended at max+1 before the phase's sweep box, its note naming the test,
the observed failure and the suspected mechanism — and that box's commit engineers the
nondeterminism out. The phase-end sweep triages open flake boxes (§11.2).

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
  **HIGH-SCRUTINY event for the G1 dual review**
  ([build-loop.md §3 step 5](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1),
  rubric item 5): the two reviewers explicitly ask **"is this suppressing a real
  regression?"**, and the (1) cite sits in the G70 `[Test-Change]` tag at the changed
  test and the (2) read-back evidence on the commit's `Tests:` line, which the
  reviewers receive in the review brief.

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
- **Determinism prerequisites:** the §7 determinism rules (injected clock, timezone
  and locale, animations off, fixed window size, fonts from the bundled set only) — a
  screenshot diff is worthless without them.

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
   door — so it takes a **G16 `proptest`** in `ipc/mod.rs` `ipc_boundary_proptest`, never
   a `cargo-fuzz` target; §1.5.)*
5. **A UI flow** → the **E2E** §5.2 walkthrough (§1.6) on the platforms the driver
   supports; **a component/hook** → **Vitest** (§1.2); **anything rendered** → the
   **axe** a11y level (§1.7).
6. **A new engine staged** → the engine's per-pair corpus + the **G26** non-
   adversarial output-validity leg + the **G32** determinism floor for its
   output-format categories, and re-run the §6.5 reliability gate before it can ship
   (§6.5.4).
7. **A build or staging script** (an uncaged `scripts/` Loop tool such as
   `stage-engines`) → its own `--selftest` legs plus the monotone leg-name pin
   (`scripts/gate-selftests/_monotone_pin.py`); the planted positive that drives the
   tool from outside is caged (`scripts/gate-selftests/**`) and lands on the box's
   `l-neg1:` route (`_format.md` §5.3) — the caged part of the box (build-loop.md
   Step 7), a named act or a sweep tail (§11.2). **A gate** → its `[GATE]` box ships
   its positive + negative G24 self-test (`_format.md` §4).

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

## 11. The phase-end Co-Pilot hardening sweep

Every phase `P2`..`P11` closes with one standing box — the phase-end Co-Pilot
hardening sweep (`[!extern]`, `[TEST]`, the last work box of its phase). Once every
other box of the phase is `[x]`, the Co-Pilot session — never the Build-Loop — runs
two legs: **Leg A** re-tests the phase's whole delivery (§11.2); **Leg B** checks the
next phase's readiness (§11.4). The box flips `[x]` when both legs are done and their
must-fix findings have landed.

### 11.1 Why a phase-level pass exists

The per-box bar (DoD item (c), §10) proves each box at its own highest sensible
level; **nobody owns the phase as a system**. The sweep attacks exactly the
residue the per-box bar structurally leaves behind (this section motivates Leg A;
Leg B's rationale is §11.4's own):

- **cross-box seams** — two boxes each correct, their composition untested;
- **whole-surface properties** — the accumulated contract surface (every IPC
  command, every event, the full state machine) exercised as one system rather
  than per-contract;
- **assumptions no single box owned** — environment, ordering, concurrency,
  platform behaviour *between* boxes;
- **the live half of structurally-untestable glue** — e.g. the §1.1a
  AppHandle-coupled boot glue, whose runtime behaviour no `cargo test` reaches,
  is exercised **live** here ahead of the scheduled §1.6 E2E infrastructure.

It is a **quality amplifier in the G1 family, not a gate** — it carries **no
`Gnn`** (judgment work a deterministic gate cannot encode; the deterministic
gates stay the only security controls), and it **composes with** the
phase-boundary G1 spot-audit cadence (security-concept §2): same boundary,
different object — the spot-audit reads review *evidence*, the sweep attacks the
delivered *system*.

### 11.2 Mandate

**Leg A — the delivery re-test.** Scope: everything the phase delivered that exists at
sweep time (code, contracts, tests, the gates it authored); a box still `[!]`-blocked at
the boundary is swept by the phase whose work unlocks it. Time box: one Co-Pilot session
plus its background jobs.

1. **Mechanical run** on the phase tree:
   - the full L4 suite;
   - `cargo-mutants` over the G15 kernel, with the run shape and on the platform the
     [`max_survived_mutants.toml`](../../max_survived_mutants.toml) header records
     (survivor counts are platform-specific). A module whose survived count exceeds its
     ratchet is a finding: in `crate::fs_guard` and `crate::detection` new tests bring it
     back to the ratchet before the flip; in `crate::outcome` the survivors become a named
     `[TEST]` box in the next phase. A survivor in code that platform does not compile is
     no test gap there: the sweep names it, and a ratchet raise it forces is an owner
     decision. The sweep's owner-acked commit lowers every ratchet whose count fell;
   - the G48 nightly fuzz results since the last sweep (every finding minimized and
     committed, §1.5);
   - flake triage (§7.1);
   - Dependabot job health, read the way roles-and-escalation §5a (*Watch health*) reads
     it: every ecosystem in `.github/dependabot.yml` has a green latest update job and none
     is paused or silent — a failing, paused or silent job is a finding;
   - the pinned action inventory: `python3 -P scripts/record-action-pins --check` (every
     committed row still reproduces) and `--resolve-images` for each digest-pinned
     `docker://` step; a moved release is re-pinned as a Co-Pilot re-land (Dependabot does
     not bump a `docker://` reference);
   - the monotone re-bless of the tool canaries, one owner-acked act per phase: after
     diffing each canary's unblessed report against the phase's box list,
     `python3 -P scripts/gate-selftests/_monotone_pin.py --bless <tool>` for
     `compile-engine-asset`, `fetch-engine-assets`, `record-action-pins` and
     `stage-engines` (a leg added since the last bless is unpinned against removal until
     then);
   - the caged tails, landed under owner-ack: the tail of every `[x]` box in scope whose
     `l-neg1:` line reads `sweep-tail`, found by a grep of the phase files
     (`plan-lint --report owner-acts` skips `[x]` boxes);
   - the phase-boundary G1 spot-audit (build-loop.md Step 5, the reviewer-family
     decision): a random ≥ 1-in-10 sample of the phase's box commits plus every `GO/GO`
     commit whose Review record carries no finding on a non-trivial diff, each record
     checked against its diff; a mismatch goes to the owner as reviewer-family evidence
     (the flip option), and the commit is never rewritten;
   - the triage of the phase's section of the [residual ledger](../plan/residual-ledger.md):
     every `Open P2/P3:` line of the phase's commits (`git log --grep='^Open P2/P3:'` over
     the phase range) is copied there and given one outcome (the ledger defines the three
     outcomes and the per-phase cap); the flip needs every line triaged, not executed. The
     same pass lists the phase's `Spec-Reconcile:` body lines
     (`git log --grep='^Spec-Reconcile:'`) for the owner's one-pass bless, which the flip
     does not wait on.
2. **Adversarial gap-hunt** over the whole delivery at the hardest technically possible
   level, with unrestricted session tooling (Docker, WebDriver/Playwright,
   property/fuzz/mutation probes, real-OS live runs, packet capture). Boxes that cross two
   or more §0.7 modules or add an IPC command, event or engine are attacked first.
   **Session-local tooling is not repo tooling:** nothing un-pinned enters the repo; a probe
   that becomes a *permanent* test lands as normal code under the existing gates and the
   pin-and-verify discipline (security-concept §0).

**Findings.** A regression or a broken invariant of a delivered contract is fixed, with
tests, before the box flips (normal commits: the DoD, G1 and §8 apply). A gap in a
capability the phase never promised becomes a named successor-phase box with its `needs:`
edge. A P2/P3 finding goes to the residual ledger. A genuine fork or spec contradiction
escalates on the normal path (roles-and-escalation §4).

**Evidence.** One findings list per landing commit body — attacked, tooling, found,
landed — inside the G11 body cap; more findings than the cap holds split the landing
commits by class. The check-off stays a bare marker flip.

**Who:** the Co-Pilot session (roles-and-escalation §1); an L(-1) surface
touched by a fix follows the normal owner-ack path (G71).

### 11.3 The mechanical boundary stop

**The next phase's first box carries a `needs:` on the sweep box** (for the
final phase, the RC sign-off box `P11.33` carries it). A `[!extern]`
prerequisite of a non-extern box STOPs that box's `needs:` closure
([`_format.md`](../plan/_format.md) §2 marker table / §6 rule 4;
roles-and-escalation §4(d)), and the sweep box is the one `[!extern]` whose
closure is the WHOLE successor phase (the 2026-09-09 scoping, build-loop.md §3
step 1, names it as its single carve-out) — so the loop **mechanically hard-stops
at every phase boundary until the sweep is `[x]`**, expressed in the existing
marker / `needs:` vocabulary plus that one named exception — which `plan-lint`
check 31 binds mechanically (exactly one sweep box per phase, `[!extern]` until
executed, last in its phase, named in `P<n+1>.1`'s `needs:`). A
DECISION-C early build of a *single* later-phase prerequisite box is **not**
gated by the boundary stop — that box is swept by its owning phase's sweep.

### 11.4 The next-phase pre-fill readiness check

**Why.** A phase's plan is written before the code it builds on exists. Leg B checks the
next phase's boxes for the classes that stop the Loop mid-phase — a caged or owner-held
deliverable inside a Loop box, an unruled fork, a mechanism nobody has run, a mandate with
no owning box — and resolves them at the boundary. A stale literal, a mis-cite, a wrong
tier or a missing embedded-type edge is not audited here: the Loop reads a box's `§§` in
full (build-loop.md Step 3) and reconciles a lower-rank text in the same commit (the
`Spec-Reconcile:` pre-check, roles-and-escalation §4(a)), and a duplicate box becomes a
`RECONCILE` when the Loop reaches it (`_format.md` §5a).

**Mechanical first**, over the P<n+1> boxes:

1. `python3 -P scripts/plan-lint` is green, and `plan-lint --report owner-acts --phase
   <n+1>` lists the phase's `[!extern]` acts and `l-neg1:` routes;
2. a grep of the boxes and the spec `§§` they cite for `[OPEN` and "before the fill" —
   each hit is ruled at the boundary (a Co-Pilot ruling or the owner's phase decision
   session) before the flip;
3. a grep of every open box's text against the `scripts/l-neg1-files.toml` globs — each
   hit gets its route: an `l-neg1:` line (`_format.md` §5.3), an item of the
   caged-preconditions act, or an `[!extern]` box; never a silent Loop deliverable.

**Targeted audit** of the boxes those greps and these surfaces select (the labels are the
ones build-gates §6 and `plan-lint` cite):

- **(e) mandate-pair and promise coverage** — every spec sentence that mandates two halves
  in one breath (a mechanism and its user-visible signal; a bound and the dispatch site it
  bounds) has an owning box for each half, and every code comment promising "X is
  P<n+1>" resolves to a real box in that phase. This surface finds MISSING boxes.
- **(f) realizability probe** — every box that names an owner or L(-1) tail, an
  acquisition mode, a signing anchor, an OS mechanism or a new dependency is RUN before it
  is ruled on, not read: the binder gate on a scratch root (does the caged half land as a
  tail, or is it a precondition?), the upstream's release and signing survey (release
  assets, attestations, the signing KEY — a web-flow-signed commit is not a maintainer
  signature), the library's build closure from its own build system, the OS probe, and
  the checker a box names, run over the live inputs it will scan, with what it read
  recorded (a check whose live input set is empty scans nothing). A box that names an
  actor names the script, its home and its semantics. The measured facts go into the
  owning spec `§` or the box, and its `needs:` / `[!extern]` shape follows from them.
- **(g) the gate catalogue's own promises** — a build-gates row that defers a leg names its
  owning `P<n>.<m>` box within a few words of the phrase; `plan-lint` check 33 refuses the
  unowned shape at L1.

**Resolution — net-negative text.** A finding resolves as a `needs:` edge, a new box, a
spec edit or a deletion of restated prose; a measured fact goes into the owning spec `§`
(or one note line within the `_format.md` §3.3 budget), never an explanatory box
paragraph. Resolvable findings land as normal plan/spec commits before the P<n+1> build
session; genuine forks go to the owner at the boundary, batched. Leg B builds nothing; an
L(-1) surface it edits follows the normal owner-ack path (G71).

**Owner acts.** Each owner act is its own `[!extern]` box that its dependents `needs:`
(`_format.md` §5a). The owner acks the phase's batch — its owner-act boxes and its
caged-preconditions act, as `plan-lint --report owner-acts --phase <n>` lists them — once,
at the phase's decision session; the Co-Pilot executes each act as soon as its Loop
prerequisites are `[x]`, grouping the ready ones, and never parks a ready act until the
last one is ready. This is the per-phase owner-act batch build-loop.md and
roles-and-escalation point to.

**The caged-preconditions act.** A phase carries at most one `[!extern]` box titled
`Land the P<m> caged preconditions …`, appended at max+1 in the P<m> file (`_format.md`
§5a) and authored at the latest by the P<m-1> sweep's Leg B (a re-cut may author it
earlier). It holds only caged items that are green on `main` before any dependent Loop box
runs — `engines.lock` rows, engine-source allow-list entries, tool pins, and CI
cache-restore steps keyed on those rows — and its dependent Loop boxes `needs:` it. It may
execute any time after authoring, in parallel with the Loop; until it is `[x]` only its
`needs:` closure waits (build-loop.md Step 1). Caged wiring that invokes a Loop-built
artifact is never in it: that is the caged part of the Loop box (build-loop.md Step 7) or
a sweep tail (§11.2).

**Scope notes.** The final phase's sweep (`P11.34`) has no successor plan; its Leg B is
vacuous.

---

## 12. References

- Technical home (the pipeline + corpus + reliability ledger):
  [spec §6.4](../spec/06-build-test-release.md) (`§6.4.1`–`§6.4.6a`), §6.5, §6.6.
- Enforcement (the gates named above): [build-gates.md](../security/build-gates.md)
  — **G8** (deferral), **G15** (unit), **G16** (bound-firing/resource fixtures),
  **G24/G24a** (gate + fixture self-test/integrity), **G26** (engine-side corpus),
  **G27/G28** (coverage floor + diff gate), **G29** (SAST/unsafe policy),
  **G30** (cross-platform build matrix the E2E flow
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
- Roles the §11 phase-end sweep binds (Co-Pilot executes it; the loop stops at
  the boundary): [roles-and-escalation.md](roles-and-escalation.md) §1, §4(d);
  the box marker + stop semantics: [`_format.md`](../plan/_format.md) §2, §6.
- Plan home: [docs/plan/P0-build-and-security.md](../plan/P0-build-and-security.md)
  §P0.5.
- Owner's core rule (completeness > pragmatism): [CLAUDE.md](../../CLAUDE.md) §6.
