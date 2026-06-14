# 04 — Formats: Documents

> Category spec for documents. Formats (SSOT *What It Converts*): **PDF, DOCX,
> DOC, ODT, RTF, TXT, MD, HTML**. Follows the per-format entry template in
> [README](README.md). One detected source type → the de-duplicated union of its
> sensible targets (§1.5); one fixed pre-highlighted default per source; one
> chosen target applies to the whole same-source batch.
>
> **PDF is documented canonically here.** This file owns the single complete PDF
> *As-target* producer list — including the cross-category producer rows
> (`pptx/ppt/odp→pdf` from [presentations.md](presentations.md) and
> `xlsx/xls/ods→pdf` from [spreadsheets.md](spreadsheets.md)) — so the PDF column
> is assembled in exactly one place. Those files reference this entry and only add
> their own `*→pdf` rows.

Every pair below is `v1-required` (full coverage, no MVP cut) and passes the SSOT
canonical inclusion test ("would a normal person plausibly want this?").
Conversions are strictly **one-source → one-target**, each satisfied by **one
engine** (no chaining, §3.2). Documents carry no patent-encumbered formats, so no
`§3.4` flags apply in this category.

---

## Source → target matrix

Rows = source, cols = target. Cell legend:
`✓` supported · `✓★` supported **and the pre-highlighted default** for that source ·
`✓~` supported but **predictably lossy** (→ §2.9) · `✓★~` default **and** lossy ·
`—` out (degenerate / no everyday demand / reverse-reconstructive) ·
`·` identity (same format, not offered as a conversion).

Engine short-names: **LO** = LibreOffice headless · **pp** = poppler `pdftotext` ·
**pd** = pandoc. (Ghostscript is **[DECIDED: NOT shipped v1]** — poppler-only PDF→TXT,
no AGPL surface; see *Engines* / §3.1.)

| src ＼ tgt | PDF | DOCX | DOC | ODT | RTF | TXT | MD | HTML |
|-----------|-----|------|-----|-----|-----|-----|----|------|
| **PDF**   | ·   | —    | —   | —   | —   | ✓★~ pp | — | —   |
| **DOCX**  | ✓★~ LO | ·  | ✓ LO | ✓ LO | ✓ LO | ✓~ pd | ✓~ pd | ✓~ pd |
| **DOC**   | ✓★~ LO | ✓ LO | · | ✓ LO | ✓ LO | ✓~ LO† | ✓~ LO† | ✓~ LO† |
| **ODT**   | ✓★~ LO | ✓ LO | ✓ LO | ·  | ✓ LO | ✓~ pd | ✓~ pd | ✓~ pd |
| **RTF**   | ✓★~ LO | ✓ LO | ✓ LO | ✓ LO | ·  | ✓~ pd | ✓~ pd | ✓~ pd |
| **TXT**   | ✓★ LO | ✓ pd | — | ✓ pd | ✓ pd | · | ✓ pd | ✓ pd |
| **MD**    | ✓★~ LO | ✓ pd | — | ✓ pd | ✓ pd | ✓~ pd | · | ✓ pd |
| **HTML**  | ✓★~ LO | ✓ pd | — | ✓ pd | ✓ pd | ✓~ pd | ✓~ pd | · |

**Reading the matrix.**
- **Everything → PDF** is the headline everyday job; PDF is the **default target
  for every other document source**. Office sources (`DOCX/DOC/ODT/RTF`) reach PDF
  via **LibreOffice**; lightweight sources (`TXT/MD/HTML`) also reach PDF via
  **LibreOffice** (it lays them out and exports the PDF in one pass — no chained
  pandoc→LaTeX step; see *Engines* for why pandoc does **not** own any `→PDF` pair).
- **PDF is a source for one target only: TXT** (text extraction). `PDF→DOCX/ODT/…`
  is reverse/reconstructive and **parked** (SSOT *Direction & shape rule*).
- **Office↔office** interconversions (`DOCX/DOC/ODT/RTF` among themselves) use
  **LibreOffice** for fidelity round-tripping.
- **Markup family** (`TXT/MD/HTML` and the office→markup *down-conversions*
  `DOCX/DOC/ODT/RTF → TXT/MD/HTML`) use **pandoc**.
- **`*→DOC`** (legacy binary Word 97-2003) is offered **only** from office sources,
  not from `TXT/MD/HTML` — nobody plausibly wants `markdown→.doc`; the modern
  `.docx` is the sole everyday Word target for those, so `TXT/MD/HTML→DOC` is `—`.

> **† `DOC → TXT/MD/HTML` is LibreOffice, NOT pandoc.** pandoc **cannot read legacy
> binary `.doc`** (and the engine notes + §3.2 + §3.5.4 say so), so these
> down-conversions are owned by **LibreOffice's** markup export filters (`Text`,
> `Markdown`, `HTML (StarWriter)`) — keeping every pair single-engine, no chaining.
> The XML/text sources (`DOCX/ODT/RTF → TXT/MD/HTML`) stay with **pandoc** (which
> reads them natively). LibreOffice's Markdown export is new in 26.2 → its
> reliability is the `[DEFER: corpus]` flag in *Category-wide* (design fixed, reliability
> empirical; `MD→PDF` parks if the gate fails — no chain-free fallback).

---

## Engines

| Engine | Role in this category | Licence | Isolation |
|--------|-----------------------|---------|-----------|
| **LibreOffice** (headless, `soffice --headless --convert-to`) | All office reads/writes (`DOCX/DOC/ODT/RTF`), and **every `*→PDF`** in the platform (this category plus the cross-category producer rows) | **MPL-2.0** (file-level copyleft; permissive enough to bundle, still shipped as a separate invoked binary per SSOT policy) | Separate sidecar process, routed through the §2.12 isolation wrapper; per-run isolated user profile (see *Edge cases*) |
| **poppler** `pdftotext` | `PDF → TXT` extraction | **`GPL-2.0-only OR GPL-3.0-only`** (valid SPDX; §3.1) | Copyleft → **separate invoked binary** (aggregation, §3.6); written-offer-of-source honored |
| **Ghostscript** | *(was: PDF read/repair plumbing behind `pdftotext`; no user-facing pair)* | **AGPL-3.0** | **[DECIDED: NOT shipped v1]** (§3.1/§3.6) — poppler-only PDF→TXT removes the AGPL surface; `[DEFER: re-add only if §6.5 corpus shows GS-salvageable PDFs]` |
| **pandoc** | Markup conversions: `MD/HTML/TXT ↔` and office→markup down-conversions (`DOCX/DOC/ODT/RTF → TXT/MD/HTML`) | **GPL-2.0+** | Copyleft → separate invoked binary (§3.6) |

**Single-engine-per-pair conformance (§3.2).** Each cell maps to exactly one
engine — no pair is reachable only by chaining:
- LibreOffice alone does `office↔office` and `*→PDF`.
- pandoc alone does the markup conversions.
- poppler alone does `PDF→TXT`.
No pair requires e.g. `MD→(LibreOffice ODT)→(pandoc HTML)`. The few places where
two engines *could* both do a job (e.g. `DOCX→HTML` via LibreOffice's HTML export
**or** pandoc) are resolved to a **single owner**: pandoc owns office→markup
(cleaner, lighter HTML/MD), LibreOffice owns office→office and `→PDF`.

Generic invocation lifecycle (spawn/progress/cancel/timeout/error-map) is owned by
§1.7; per-engine concrete argument construction lives in §3.5. The values below
are the **concrete option lists and defaults** this file owns (§1.6).

---

## Per-format entries

### `PDF` — Portable Document Format *(canonical home)*

- **Detection:** magic bytes `25 50 44 46 2D` (`%PDF-`) at offset 0, optionally
  preceded by a small junk prefix some producers emit (scan a short window).
  Extension `.pdf`. MIME `application/pdf`. Trailer `%%EOF`. Unambiguous.
- **Role:** **both** — but a *narrow* source (one target) and a *broad* target.
- **As source → targets:**
  | Target | Engine | Default | Lossy | Notes |
  |--------|--------|:------:|:-----:|-------|
  | **TXT** | poppler `pdftotext` | ★ | ✓ (layout, images, tables) | The only sensible PDF source pair (text extraction). |

  `PDF → DOCX/ODT/HTML/MD/RTF/DOC` are **out of v1**: reverse/reconstructive
  (SSOT *Direction & shape rule*), and OCR of scanned/image PDFs is explicitly
  Parked. A normal person's PDF wish is "get the text out" → TXT covers it.
- **As target ← sources (THE single complete producer list):**
  | Producer (source) | Engine | Owning category file | Lossy |
  |-------------------|--------|----------------------|:-----:|
  | `DOCX → PDF` | LibreOffice | documents (here) | ✓ (reflow) |
  | `DOC → PDF`  | LibreOffice | documents (here) | ✓ (reflow) |
  | `ODT → PDF`  | LibreOffice | documents (here) | ✓ (reflow) |
  | `RTF → PDF`  | LibreOffice | documents (here) | ✓ (reflow) |
  | `TXT → PDF`  | LibreOffice | documents (here) | — (faithful) |
  | `MD → PDF`   | LibreOffice | documents (here) | ✓ (reflow) |
  | `HTML → PDF` | LibreOffice | documents (here) | ✓ (rendering differences) |
  | `PPTX → PDF` | LibreOffice | [presentations.md](presentations.md) | ✓ (animations/transitions/embedded media dropped) |
  | `PPT → PDF`  | LibreOffice | [presentations.md](presentations.md) | ✓ (animations/transitions/embedded media dropped) |
  | `ODP → PDF`  | LibreOffice | [presentations.md](presentations.md) | ✓ (animations/transitions/embedded media dropped) |
  | `XLSX → PDF` | LibreOffice | [spreadsheets.md](spreadsheets.md) | ✓ (page-break/scaling: large sheets paginate) |
  | `XLS → PDF`  | LibreOffice | [spreadsheets.md](spreadsheets.md) | ✓ (page-break/scaling) |
  | `ODS → PDF`  | LibreOffice | [spreadsheets.md](spreadsheets.md) | ✓ (page-break/scaling) |

  Every PDF producer in the entire app is in this one table. The cross-category
  rows reference their owning files for source-side detection/options but the PDF
  *column* is assembled here so the matrix can never be split or contradicted.
- **Engine(s):**
  - *Produce PDF:* **LibreOffice** headless, filter `writer_pdf_Export` (Writer),
    `calc_pdf_Export` (Calc), `impress_pdf_Export` (Impress). MPL-2.0.
  - *Consume PDF (→TXT):* **poppler `pdftotext`** (GPL) only. (Ghostscript is
    **[DECIDED: NOT shipped v1]** — no GS fault-tolerance backstop; poppler-only,
    fail-clearly on the rare unrecoverable PDF. §3.1.)
- **Options/settings (PDF as the *output* of `*→PDF`):** ConvertIA exposes **none**
  by default — "it just works" (Principle 8). Internal fixed defaults passed to
  the export filter:
  | Setting | Default | Range/values | Surfaced? |
  |---------|---------|--------------|-----------|
  | `SelectPdfVersion` | `0` (PDF 1.7, max compatibility) | `0`=PDF 1.7 (default; **no version restriction**), `1`=PDF/A-1b, `2`=PDF/A-2b, `3`=PDF/A-3b, `15`=PDF 1.5, `16`=PDF 1.6, `17`=PDF 1.7 (verified against the official LibreOffice `pdf_params` reference, LO `writer_pdf_Export`/`impress_pdf_Export` — `15/16/17` are **plain PDF versions, NOT PDF/A**; the PDF/A levels are `1/2/3`. The earlier `15/16/17`→PDF/A mapping was WRONG and would have silently emitted plain PDF instead of PDF/A) | no |
  | `UseTaggedPDF` | `true` (accessibility: structure/headings) | bool | no |
  | `ReduceImageResolution` | `false` (preserve embedded image quality) | bool | no — see "compress PDF" (`[DECIDED]` out of v1) |
  | `Quality` (JPEG) | `90` | 1–100 | no |
  | `ExportBookmarks` | `true` | bool | no |
  | Page range | all pages | — | no |
  No "Advanced options" panel ships for documents in v1 (SSOT: adding a setting is
  a scope change). The single candidate future toggle ("compress / smaller PDF")
  is `[DECIDED]` out of v1 — tracked in *Category-wide* (`[DEFER: post-v1]`).
- **Options (PDF as *source*, → TXT):** see the TXT entry.
- **Lossy?:** As a **target**, `*→PDF` from word-processor sources is *reflow*
  lossy (→ §2.9 `doc_pdf_reflow`); from `TXT` it is faithful. As a
  **source**, `PDF→TXT` is heavily lossy (→ §2.9 `doc_pdf_to_text`).
- **Edge cases:**
  - **Password-protected / encrypted PDF → OUT OF SCOPE.** ConvertIA does not
    prompt for or crack passwords. An encrypted PDF reaching `PDF→TXT` is detected
    (poppler reports encryption) and **fails clearly** ("this PDF is
    password-protected — ConvertIA can't read it") per §2.8 — never a crash, never
    a silent empty output.
  - **Scanned / image-only PDF → TXT** yields little or no text (no OCR in v1). The
    `pdf→txt` lossy note already warns "text only"; an essentially-empty
    extraction is reported, not surfaced as a misleading success of an empty file.
  - **Malformed / truncated PDF:** poppler tolerates many; an unrecoverable PDF →
    fail clearly (§2.8), batch continues (§1.9). (No Ghostscript repair backstop in
    v1 — GS dropped, §3.1; `[DEFER: re-add only if §6.5 shows GS-salvageable PDFs]`.)
  - **PDF forms (AcroForm/XFA), tagged structure, layers:** flattened to their
    visible text on `→TXT`; not reconstructed.
  - **Very large PDF:** sized in pre-flight (§1.10); progress is real per-item.

---

### `DOCX` — Office Open XML Word document

- **Detection:** ZIP container (`50 4B 03 04`) whose archive contains
  `word/document.xml` and `[Content_Types].xml` with WordprocessingML content
  type. Extension `.docx`. **Ambiguity:** all OOXML/ODF/`.epub` files share the
  ZIP magic — detection MUST inspect the archive's content type, not the magic
  alone (§1.2), to distinguish `DOCX` vs `XLSX` vs `PPTX` vs `ODT`.
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy | Notes |
  |--------|--------|:------:|:-----:|-------|
  | **PDF** | LibreOffice | ★ | ✓ reflow | The everyday "send me a PDF" job. |
  | DOC | LibreOffice | | — (minor feature loss) | Legacy Word for old recipients. |
  | ODT | LibreOffice | | — | Open-document equivalent. |
  | RTF | LibreOffice | | ✓ (rich features simplified) | Universal word-processor exchange. |
  | TXT | pandoc | | ✓ (formatting/images dropped) | Plain text only. |
  | MD | pandoc | | ✓ (layout/styling dropped) | Markdown skeleton (headings/lists/links/tables). |
  | HTML | pandoc | | ✓ (page layout dropped) | Web-ready; images extracted/`--embed-resources`. |
- **As target ← sources:** `DOC, ODT, RTF` (LibreOffice); `TXT, MD, HTML`
  (pandoc). Not `PDF` (reverse/parked).
- **Engine(s):** LibreOffice for office targets + PDF; pandoc for markup targets.
- **Options/settings:** none surfaced. pandoc down-conversions use fixed defaults:
  `--wrap=preserve`; `DOCX→HTML` uses `--embed-resources --standalone` (images
  inlined as data URIs so the single HTML file is portable — honors *content
  fidelity*); `DOCX→MD` writes GitHub-Flavored Markdown (`-t gfm`) with
  `--extract-media` disabled in favor of referencing — `[DEFER: corpus]` image policy below.
- **Lossy?:** `→PDF` reflow (§2.9 `doc_pdf_reflow`); `→TXT` (§2.9 `doc_to_text`),
  `→MD/RTF` (§2.9 `doc_simplified`) drop progressively more formatting. `→HTML`
  (§2.9 `doc_simplified`). `→DOC/ODT` near-lossless.
- **Edge cases:** **fonts** — if a document font isn't embedded and isn't on the
  system, LibreOffice substitutes (metrics shift → reflow); ConvertIA bundles a
  baseline metric-compatible font set to minimize this (see *Category-wide*).
  **Embedded images** preserved into PDF/ODT/DOC/RTF; into HTML extracted/inlined;
  dropped for TXT/MD-without-media. **Tracked changes/comments** are rendered per
  the document's display state, not specially exported. **Macros** never executed
  (headless, sandboxed). **Encoding** is internal (XML/UTF-8) — non-Latin/RTL text
  preserved.

---

### `DOC` — legacy Microsoft Word 97–2003 (binary)

- **Detection:** OLE2 Compound File Binary magic `D0 CF 11 E0 A1 B1 1A E1`,
  containing a `WordDocument` stream. Extension `.doc`. **Ambiguity:** the OLE2
  magic is shared by legacy `.xls` and `.ppt` — detection inspects the internal
  stream directory to disambiguate (§1.2).
- **Role:** **both**.
- **As source → targets:** same target *set* as DOCX, but **TXT/MD/HTML are owned
  by LibreOffice, not pandoc** (pandoc can't read binary `.doc`) —
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | ✓ reflow |
  | DOCX | LibreOffice | | — (modernize) |
  | ODT | LibreOffice | | — |
  | RTF | LibreOffice | | ✓ |
  | TXT | **LibreOffice** (`Text`) | | ✓ |
  | MD | **LibreOffice** (`Markdown`, 26.2) | | ✓ |
  | HTML | **LibreOffice** (`HTML (StarWriter)`) | | ✓ |
- **As target ← sources:** `DOCX, ODT, RTF` (LibreOffice). **Not** from
  `TXT/MD/HTML` (no everyday demand for `markdown→.doc` — see matrix note). Not PDF.
- **Engine(s):** LibreOffice reads legacy `.doc` natively; pandoc cannot read
  binary `.doc` — therefore `DOC→TXT/MD/HTML` go through **pandoc only if pandoc
  can read it**. pandoc **cannot** read legacy `.doc`. **Resolution (single-engine
  rule):** `DOC→TXT/MD/HTML` is owned by **LibreOffice's** markup export filters
  (`Text`, `Markdown`†, `HTML (StarWriter)`) — *not* pandoc — so no chaining is
  needed. († LibreOffice Markdown export is new in 26.2; `[DEFER: corpus]` reliability flag
  in *Category-wide* — design fixed, only reliability empirical.)
- **Options/settings:** none surfaced.
- **Lossy?:** same profile as DOCX.
- **Edge cases:** Old binary `.doc` with unusual code pages — LibreOffice handles
  legacy encodings; verified against the non-Latin/RTF corpus (SSOT DoD). Embedded
  OLE objects (e.g. old equation editor) may not render — reported, not crashed.

> **Note (engine ownership correction):** because pandoc cannot read legacy binary
> `.doc`, the markup down-conversions **from DOC** (`DOC→TXT/MD/HTML`) are assigned
> to **LibreOffice**, while the same down-conversions from the XML/text sources
> (`DOCX/ODT/RTF→TXT/MD/HTML`) stay with **pandoc** (which reads those natively).
> This keeps every pair single-engine. See the matrix engine annotations.

### `ODT` — OpenDocument Text

- **Detection:** ZIP container; uncompressed first entry `mimetype` =
  `application/vnd.oasis.opendocument.text`. Extension `.odt`. The leading
  `mimetype` member makes ODF unambiguous vs OOXML.
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | ✓ reflow |
  | DOCX | LibreOffice | | — |
  | DOC | LibreOffice | | — (feature loss) |
  | RTF | LibreOffice | | ✓ |
  | TXT | pandoc | | ✓ |
  | MD | pandoc | | ✓ |
  | HTML | pandoc | | ✓ |
- **As target ← sources:** `DOCX, DOC, RTF, TXT, MD, HTML`. (`TXT/MD/HTML→ODT`
  via pandoc; office sources via LibreOffice.)
- **Engine(s):** LibreOffice native format; pandoc reads ODT for markup targets.
- **Options/settings:** none surfaced.
- **Lossy?:** `→PDF` reflow; `→RTF/markup` simplification.
- **Edge cases:** as DOCX (fonts, images, encoding). ODT is LibreOffice's home
  format → highest-fidelity office round-trips.

### `RTF` — Rich Text Format

- **Detection:** ASCII magic `7B 5C 72 74 66` (`{\rtf`) at offset 0. Extension
  `.rtf`. Text-based, unambiguous.
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | ✓ reflow |
  | DOCX | LibreOffice | | — |
  | DOC | LibreOffice | | — |
  | ODT | LibreOffice | | — |
  | TXT | pandoc | | ✓ |
  | MD | pandoc | | ✓ |
  | HTML | pandoc | | ✓ |
- **As target ← sources:** `DOCX, DOC, ODT` (LibreOffice); `TXT, MD, HTML`
  (pandoc). RTF is the "universal" word-processor interchange, so it is both a
  common source and a common target.
- **Engine(s):** LibreOffice for office round-trips + PDF; pandoc reads RTF
  (RTF reader added to recent pandoc) for `RTF→TXT/MD/HTML`. **Fallback note:**
  pandoc's RTF reader has known gaps (super/subscript, complex tables); if it is
  judged unreliable on the corpus, ownership of `RTF→TXT/MD/HTML` falls back to
  **LibreOffice's** markup export — `[DEFER: corpus]` recorded in *Category-wide* (item 2).
- **Options/settings:** none surfaced.
- **Lossy?:** `→PDF` reflow; markup simplification.
- **Edge cases:** RTF embeds images as hex/encoded blobs — preserved into office
  targets/PDF; extracted (pandoc) or dropped for TXT. Code-page declarations in
  the RTF header drive encoding — handled by the reader so non-Latin text survives.

### `TXT` — plain text

- **Detection:** **no magic** — TXT is the absence of a recognized binary
  signature plus valid text decoding. Detection (§1.2) treats a file as TXT when
  content sniffing finds no known format and the bytes decode cleanly as text
  (UTF-8/UTF-16 BOM honored, else charset-detected: UTF-8 → Windows-1252/Latin-1
  → others). Extension `.txt` (also extension-less text files). Distinguishing
  `TXT` vs `MD` is by **extension/intent**, not content (Markdown is valid plain
  text); a `.md` is MD, a `.txt` is TXT.
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | — (faithful: monospaced/flowed text) |
  | DOCX | pandoc | | — |
  | ODT | pandoc | | — |
  | RTF | pandoc | | — |
  | MD | pandoc | | — (passthrough; light structuring) |
  | HTML | pandoc | | — (`<pre>`/paragraphs) |
- **As target ← sources:** **every** other document source (`PDF, DOCX, DOC, ODT,
  RTF, MD, HTML`) — "just give me the words" is the universal down-conversion.
  From PDF via poppler; from DOC via LibreOffice; from DOCX/ODT/RTF/MD/HTML via
  pandoc.
- **Engine(s):** `TXT→PDF` LibreOffice (lays text into pages); `TXT→other markup/
  office` pandoc (reads input as plain/markdown, writes target).
- **Options/settings:** **encoding on output is fixed to UTF-8** (with no BOM by
  default) — content-fidelity guarantee (§2.10). No surfaced switch in v1.
  `[DECIDED]` NOT in v1: an "output encoding" advanced toggle is not offered
  (`[DEFER: post-v1]`) — UTF-8 is the right default for everyone.
- **Lossy?:** `TXT→*` is **not** lossy (plain text has nothing to lose); only the
  *reverse* (`*→TXT`) is lossy.
- **Edge cases:** **encoding** is the whole game — input charset is detected, and
  CR/LF vs LF line endings are normalized on the target's terms. Mixed-encoding
  or invalid byte sequences → fail clearly rather than emit mojibake. Extremely
  long lines / huge logs handled (streamed where the engine allows).

### `MD` — Markdown

- **Detection:** **no magic** — text file; identified by extension
  `.md`/`.markdown` (and the §1.2 text-sniff). Treated as **CommonMark / GitHub-
  Flavored Markdown** on read.
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | ✓ reflow (`doc_pdf_reflow`, §2.9 — LO lays MD out with font-substitution/reflow like every other word-processor `→PDF`) |
  | HTML | pandoc | | — (the natural rendering) |
  | DOCX | pandoc | | — |
  | ODT | pandoc | | — |
  | RTF | pandoc | | — |
  | TXT | pandoc | | ✓ (strips markup syntax → plain prose) |
- **As target ← sources:** `DOCX, DOC, ODT, RTF, HTML, TXT` — extract a
  Markdown skeleton from richer documents (headings, lists, links, tables, basic
  emphasis). From DOC via LibreOffice; others via pandoc.
- **Engine(s):** `MD→PDF` LibreOffice (renders Markdown to a laid-out PDF, via LO 26.2's
  new native Markdown *import*); `MD→HTML/DOCX/ODT/RTF/TXT` pandoc. **`MD→PDF` engine
  `[DEFER: corpus]`** (design fixed = LO 26.2 Markdown import; only its reliability is
  empirical) — see below.
  **No chain-free fallback for `MD→PDF` `[DECIDED — flag explicitly]`:** `MD→DOCX/ODT/RTF`
  fall back to **pandoc** (single engine), but **`MD→PDF` has NO single-engine fallback** —
  the `MD→pandoc-HTML→LibreOffice-PDF` chain is **explicitly disallowed** (no chains). So if
  the LO 26.2 Markdown-import corpus gate **fails**, `MD→PDF` must be **demoted to parked**
  (per the SSOT v1-DoD second exception — a pair may be parked rather than shipped broken),
  **not** silently routed through a chain. Phase 3 must **not** assume a silent fallback
  exists for `MD→PDF`; the corpus result decides ship-vs-park.
- **Options/settings:** none surfaced. pandoc input dialect fixed to `gfm`
  (GitHub-Flavored: tables, task lists, strikethrough, autolinks) — the dialect a
  normal person's `.md` most likely is. `MD→HTML` uses `--standalone
  --embed-resources` (self-contained page) by default.
- **Lossy?:** `MD→PDF` is **reflow-lossy `[DECIDED]` (`✓★~`, §2.9 `doc_pdf_reflow`)** — LO
  lays Markdown out into pages with **font substitution + reflow** exactly like every other
  word-processor `→PDF-via-LO` path (DOCX/DOC/ODT/RTF/HTML→PDF are all `✓★~`), so MD→PDF is
  classified the SAME, **not** "faithful" like the structureless `TXT→PDF` case. `MD→HTML/DOCX/
  ODT/RTF` faithful; `MD→TXT` strips syntax (lossy, §2.9 `doc_to_text`).
- **Edge cases:** embedded image references (`![](path)` / remote URLs) — **local**
  relative images are resolved/embedded; **remote** URLs are *not* fetched (SSOT
  *fully offline / no network*) — they become broken references and this is noted.
  Raw HTML inside Markdown is passed through by pandoc. Fenced code blocks render
  monospaced. Front-matter (YAML) is parsed as metadata, not printed as text.

### `HTML` — HyperText Markup Language

- **Detection:** text; sniff for `<!DOCTYPE html`, `<html`, or a leading `<` with
  HTML-ish tags (case-insensitive, BOM/whitespace tolerant). Extensions
  `.html`/`.htm`. **Single-file HTML only** in v1 (a folder of HTML + assets is
  not a "document").
- **Role:** **both**.
- **As source → targets:**
  | Target | Engine | Default | Lossy |
  |--------|--------|:------:|:-----:|
  | **PDF** | LibreOffice | ★ | ✓ (rendering/CSS differences) |
  | DOCX | pandoc | | — |
  | ODT | pandoc | | — |
  | RTF | pandoc | | ✓ |
  | TXT | pandoc | | ✓ (tags stripped → plain text) |
  | MD | pandoc | | ✓ (rich HTML simplified to Markdown) |
- **As target ← sources:** `DOCX, DOC, ODT, RTF, MD, TXT`. From DOC via
  LibreOffice; from MD/TXT/DOCX/ODT/RTF via pandoc.
- **Engine(s):** `HTML→PDF` **LibreOffice** (its HTML import filter renders to a
  laid-out PDF in one pass — no headless-Chromium/wkhtmltopdf needed, keeping the
  bundle lean and the pair single-engine). `HTML→office/markup` pandoc.
- **Options/settings:** none surfaced.
- **Lossy?:** `HTML→PDF` is lossy in the sense that LibreOffice's HTML/CSS engine
  is **not** a full modern browser — complex CSS/JS-driven layouts will differ
  (§2.9 `doc_html_render`). `HTML→TXT/MD` drop styling (§2.9 `doc_to_text` /
  `doc_simplified`). Simple,
  document-like HTML (articles, reports) converts faithfully.
- **Edge cases:** **JavaScript is never executed** — only static HTML is rendered
  (offline + security). **External CSS/images** referenced by remote URL are
  **not fetched** (offline) → styled/visual gaps, noted; relative local assets are
  resolved. Character encoding from `<meta charset>` / BOM honored. Embedded
  `<svg>` and data-URI images render; remote `<img src=http…>` do not.

---

## Category-wide

### Per-source default target (one-glance summary)

| Source | Pre-highlighted default | Why (SSOT tie-break: widely-compatible everyday target) |
|--------|------------------------|----------------------------------------------------------|
| **PDF** | **TXT** | The only sensible derivative; "get the text out". |
| **DOCX** | **PDF** | The universal "share a final document" target. |
| **DOC** | **PDF** | Same — and a clean way off the dead legacy format. |
| **ODT** | **PDF** | Same. |
| **RTF** | **PDF** | Same. |
| **TXT** | **PDF** | "Make my notes a proper document." |
| **MD** | **PDF** | The everyday "render my Markdown to share it". |
| **HTML** | **PDF** | "Save this page as a document." |

Every document source defaults to **PDF**, *except PDF itself*, which defaults to
**TXT**. This is the single most predictable everyday behavior and lets the common
path stay *drop → (PDF already highlighted) → convert* in two clicks (Principle 8).

### Fonts (embedding & substitution)

- LibreOffice **embeds** fonts into produced PDFs where licensing flags allow,
  so the PDF looks the same on any viewer.
- When a *source* document references a font that is neither embedded nor present,
  LibreOffice substitutes a metric-compatible face → minor reflow (this is the
  primary cause of the §2.9 `doc_pdf_reflow` lossy note). ConvertIA bundles a
  **baseline open font set** (a Liberation-class metric-compatible family covering
  the common Arial/Times/Courier metrics, plus broad Unicode coverage incl. CJK
  and RTL) with the LibreOffice sidecar so substitution is graceful and non-Latin
  text never tofu's. **The bundled font set is `[DECIDED]` at §3.9.3 — Liberation +
  Carlito + Caladea + a curated Noto CJK/RTL subset** (the §6.4.5 corpus font floor the
  CJK/RTL fidelity gate tests against); only the **CJK breadth** remains `[DEFER: size]`
  (which Noto CJK weights/scripts, a size-budget question, not a design one).

### Encoding & content fidelity (SSOT *Content fidelity*)

- All text **output** defaults to **UTF-8** (no BOM unless the target demands it).
- Input charset is **detected** (BOM → declared `<meta>`/RTF code page → heuristic
  UTF-8/Windows-1252/Latin-1 → broader detection), never assumed from extension.
- CJK and right-to-left scripts (Arabic/Hebrew) pass through intact in every
  engine path; this is part of the v1 reliability corpus (SSOT DoD).

### Images & embedded objects

- Into **PDF / office targets**: embedded raster/vector images are preserved.
- Into **HTML** (pandoc): images are **inlined** (`--embed-resources`) so the
  single output file is self-contained — no sidecar asset folder, honoring the
  one-file→one-file model.
- Into **TXT** and bare **MD**: images are dropped (TXT) or referenced, never
  silently exported to loose files. **`[DEFER: corpus]`:** the exact `DOCX/ODT→MD` image
  policy — inline as base64 data URIs vs reference vs drop — leans **drop with a
  note** for MD (data-URI-bloated Markdown is ugly); the lean is fixed and only the
  call is validated against real `.docx`/`.odt` corpus files (Category-wide item 4).

### Lossy disclosure (links to §2.9 — strings live there, not here)

Predictably-lossy pairs in this category, each mapped to the exact §2.9
`LossyKind` (the catalog owns the string; this file only names the kind):
- `PDF → TXT` → §2.9 `doc_pdf_to_text`.
- `* → PDF` from word-processor sources (`DOCX/DOC/ODT/RTF`) **and `MD → PDF`** (LO lays
  Markdown out with reflow/font-substitution, same as the word-processor sources) → §2.9
  `doc_pdf_reflow`.
- `HTML → PDF` → §2.9 `doc_html_render`.
- `* → TXT` (from DOCX/DOC/ODT/RTF/MD/HTML) → §2.9 `doc_to_text`.
- `* → MD` and `* → RTF` from rich sources → §2.9 `doc_simplified`.
- `TXT → PDF/HTML/office` and `MD → HTML/office` are **not** flagged (faithful). **`MD → PDF`
  IS flagged `doc_pdf_reflow`** (the one MD→PDF exception — LO reflows it, see above);
  `TXT → PDF` stays faithful because plain text has no structure to reflow.

The note is a calm, passive inline line next to the chosen target (Principle 7),
shown only for these predictable cases — never a blocking dialog or per-conversion
nag.

### Out-of-scope / parked (honestly surfaced)

- **Password-protected / encrypted PDF** → out of scope (fail clearly, never crack).
- **Reverse/reconstructive** `PDF→DOCX/ODT/HTML/…` → Parked (SSOT *Direction rule*).
- **OCR** (scanned PDF → searchable text) → Parked.
- **Multi-page PDF → one image per page** (and any one-to-many fan-out) → Parked
  (SSOT *Future Ideas*); ConvertIA stays strictly one-file→one-file.
- **`TXT/MD/HTML → DOC`** (legacy binary Word) → out (no everyday demand; `.docx`
  is the modern Word target for these sources).
- **Multi-file HTML site / HTML+assets folder** → out (not a single document).

### Open / deferred decisions

1. **`MD→PDF` and `MD→ODT/DOCX/RTF` engine ownership — `[DEFER: corpus]`.** Native
   LibreOffice Markdown *import* landed only in **LibreOffice 26.2 (Mar 2026)** and
   is unproven on the v1 corpus. The v1 default is **(a) LibreOffice imports `.md`
   and exports PDF/ODT/DOCX directly** (single-engine); the documented fallback,
   **only if the corpus shows LO MD import unreliable**, is **(b) pandoc owns
   `MD→DOCX/ODT/RTF/HTML/TXT`**. A `MD→(pandoc HTML)→(LO PDF)` chain is
   **disallowed** (§3.2). **`MD→PDF` has NO chain-free fallback `[DECIDED]`:** unlike
   `MD→DOCX/ODT/RTF` (which fall back to pandoc), `MD→PDF` can ONLY be served by LO's
   Markdown import — pandoc has no single-engine PDF path here without the disallowed chain.
   So **if the LO 26.2 corpus gate fails, `MD→PDF` is DEMOTED TO PARKED** (per the SSOT
   v1-DoD second exception), not chained and not shipped broken. Genuinely empirical →
   deferred to corpus validation, not an open design question.
2. **`RTF→TXT/MD/HTML` engine ownership — `[DEFER: corpus]`.** pandoc owns the RTF
   down-conversions **unless** corpus testing shows its RTF reader too lossy
   (super/subscript, complex tables), in which case **LibreOffice** takes them.
   (`DOC→TXT/MD/HTML` is **already DECIDED LibreOffice** — pandoc can't read binary
   `.doc`; see the matrix `LO†` cells and the engine-ownership note.) Empirical →
   deferred.
3. **Ghostscript bundling — `[DECIDED]`: dropped in v1** (poppler-only `PDF→TXT`, no
   AGPL surface; §3.1/§3.6). **[DEFER:** re-add only if the §6.5 corpus shows poppler
   failing PDFs GS would salvage.**]**
4. **`*→MD` image policy — `[DEFER: corpus]`** (see *Images* above) — drop-with-note
   (lean) vs data-URI inline; resolve against real `.docx`/`.odt` corpus files.
5. **Bundled font set — `[DECIDED]` baseline in §3.9.3** (Liberation+Carlito+Caladea
   + curated Noto CJK/RTL subset); only the CJK breadth is **[DEFER: size]**.
6. **"Compress / smaller PDF" toggle — `[DECIDED]` out of v1** (the one plausible future
   Advanced option for `*→PDF`: `ReduceImageResolution`/`Quality`). Out by the "adding a
   setting is a scope change" rule; recorded so it isn't lost (`[DEFER: post-v1]`).
