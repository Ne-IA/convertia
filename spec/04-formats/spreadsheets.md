# 04 — Formats: Spreadsheets

> Formats (SSOT *What It Converts*): **XLSX, XLS, ODS, CSV, TSV** — plus **PDF**
> as a derived target only (xlsx/xls/ods → PDF). Follows the per-format template
> in [README](README.md).
>
> **Single owner note:** PDF is documented **canonically in
> [documents.md](documents.md)**. This file does **not** re-document PDF; it only
> declares the spreadsheet→PDF *producer* rows and references the canonical PDF
> entry for detection, options, and the full as-target enumeration. The general
> "one detected type → de-duplicated union of targets" rule is owned by §1.5; the
> generic option-declaration model by §1.6; the lossy string catalog by §2.9.

---

## Category intent (the everyday demand)

A spreadsheet category for normal people, not analysts. The everyday wants are:

- **Modernise / open an old workbook** — `XLS → XLSX` (and `XLS → ODS`), so a
  legacy `.xls` opens cleanly in current tools.
- **Office ↔ open format** — `XLSX ↔ ODS` (move between Excel and
  LibreOffice/OpenOffice).
- **Get the data out as plain text** — `* → CSV` / `* → TSV` (feed a database,
  a script, an import wizard, a "comma-separated" upload field).
- **Bring plain text into a real workbook** — `CSV / TSV → XLSX` (and `→ ODS`,
  `→ XLS`) so a delimited dump becomes a tidy, formatted sheet.
- **Hand someone a frozen, printable copy** — `XLSX / XLS / ODS → PDF`.

Everything here passes the SSOT inclusion test ("would a normal person plausibly
want this?"). Pairs that fail it are marked **out** in the matrix with a reason
(e.g. `CSV → TSV` is a degenerate "swap one delimiter" that the import wizard of
any target already does — kept **in** because re-delimiting a text dump *is* a
real everyday ask; `PDF → spreadsheet` is reverse/reconstructive and **out** per
the SSOT *Direction & shape rule*).

All conversions are strictly **one-source → one-target** and each pair is
satisfied by **one** engine (§3.2) — no chaining.

---

## Source → target matrix

Rows = source, cols = target. Cell legend:

- `✓` supported · `✓★` supported **and the pre-highlighted default** target for
  that source · `✓~` supported but **predictably lossy** (see §2.9) · `—` not
  offered (degenerate / out of scope, reason in notes) · short engine tag in
  parentheses.
- Engine tags: **LO** = LibreOffice headless (`soffice`); **native** = ConvertIA's
  built-in Rust CSV/TSV text engine (no LO process). See [Engines](#engines).

| src ＼ tgt | XLSX | XLS | ODS | CSV | TSV | PDF |
|-----------|------|-----|-----|-----|-----|-----|
| **XLSX**  | —¹   | ✓ (LO) | ✓ (LO) | ✓~★ (LO) | ✓~ (LO) | ✓~ (LO) |
| **XLS**   | ✓★ (LO) | —¹ | ✓ (LO) | ✓~ (LO) | ✓~ (LO) | ✓~ (LO) |
| **ODS**   | ✓★ (LO) | ✓ (LO) | —¹ | ✓~ (LO) | ✓~ (LO) | ✓~ (LO) |
| **CSV**   | ✓★ (LO) | ✓ (LO) | ✓ (LO) | —¹ | ✓ (native) | —² |
| **TSV**   | ✓★ (LO) | ✓ (LO) | ✓ (LO) | ✓ (native) | —¹ | —² |

¹ **Same-format identity cell** — not a conversion the user picks. Re-saving a
file as its own format is not offered as a target (it would only ever be used to
re-encode, which is not an everyday spreadsheet ask). *(Re-delimiting CSV↔TSV is
the genuine "same-ish text" case and **is** offered, as separate formats.)*

² **CSV/TSV → PDF is out.** A raw delimited text file has no page layout,
column widths, or styling; "printing" it to PDF produces a monospaced text dump
that is not what a normal person wants and is better served by `CSV → XLSX →`
(user then prints). One-source→one-target + no-chaining rule means we would have
to route CSV→PDF through LO's *Calc* import anyway; the everyday-demand payoff is
too low. Marked **out** (parked candidate if demand appears). If a user wants a
PDF of tabular text, the in-app default path is `CSV → XLSX` first.

**Lossy cells (`✓~`)** are every `* → CSV/TSV` (formatting/formulas/multi-sheet
dropped) and every `* → PDF` (live workbook → frozen page; possible reflow/font
substitution). All link to §2.9 — see [Lossy](#lossy-disclosure).

**PDF column:** the PDF *target* itself (detection, page options, full
as-target source list) lives in **[documents.md](documents.md)**; the three
producer rows above (XLSX/XLS/ODS → PDF) are declared there as part of PDF's
canonical as-target enumeration. This file only owns the spreadsheet-side
options for the producer (page orientation, fit-to-width, sheet selection).

---

## Engines

| Engine | Use | Licence | Platform | Patent |
|--------|-----|---------|----------|--------|
| **LibreOffice headless** (`soffice --headless --convert-to`) | All XLSX/XLS/ODS reads & writes; CSV/TSV when a *spreadsheet* is on either side (so cells become a real table, not raw text); all `* → PDF` | **MPL-2.0** (+ bundled components, see §3.7 SBOM) | Win/macOS/Linux | none |
| **Native CSV/TSV text engine** (Rust, in-core) | `CSV ↔ TSV` only (pure text re-delimiting); plus **front-of-house detection** of encoding & delimiter for any CSV/TSV input before it is handed to LO | MIT (ConvertIA's own code) | all | none |

**Why two engines, single-owner per pair.** §3.2 requires exactly one engine per
(source,target). The split is along a clean line:

- A pair where **a spreadsheet binary is on at least one side**
  (`xlsx→csv`, `csv→xlsx`, `xlsx→ods`, `ods→pdf`, …) is owned by **LibreOffice**
  — only a real spreadsheet engine can parse `.xlsx`/`.ods` zip+XML or write a
  styled workbook.
- The **`CSV ↔ TSV`** pair (text-in, text-out, no workbook) is owned by the
  **native** engine: it is a single-pass encoding-normalise + delimiter-swap that
  does not need a 300 MB office process, is faster, and never risks LO's
  number/date *auto-recognition* mangling values (e.g. turning `0123` into `123`,
  or `3/4` into a date — the classic CSV-through-a-spreadsheet data-corruption
  trap). Keeping CSV↔TSV out of LO is a **content-fidelity** decision (SSOT
  *Content fidelity*), not just performance.

> No chaining: every cell in the matrix is reachable by one of these two engines
> directly. There is no pair that needs `A → (intermediate) → B`.

**LibreOffice invocation shape** (concretes in §3.5): one headless `soffice`
per item, isolated per §2.12, output captured then atomically placed per §2.1.
The CSV export filter name is `Text - txt - csv (StarCalc)`; the import filter
for delimited text into Calc is the same filter with import `FilterOptions`.
Profile/user-dir is per-run and disposable (no shared lock between concurrent
items — see §1.7 / §2.6).

---

## Per-format entries

### `XLSX` — Office Open XML Spreadsheet

- **Detection:** ZIP container (`50 4B 03 04`) whose `[Content_Types].xml`
  declares `…spreadsheetml…`; the OPC part `xl/workbook.xml` is present.
  Extension `.xlsx` (also `.xlsm` macro-enabled — see edge cases). Because the
  magic bytes are generic ZIP, detection **must** peek inside the container, not
  trust the extension (SSOT *Recognize files by content*); a `.xlsx` that is
  really a DOCX or an ODS is classified by its inner manifest, not its name.
- **Role:** both (source **and** target).
- **As source → targets:** `XLS`, `ODS`, **`CSV` ★(default)**, `TSV`, `PDF`
  — all via LibreOffice.
- **As target ← sources:** `XLS`, `ODS`, `CSV`, `TSV` (LibreOffice). *(Not from
  PDF — reverse direction is out.)*
- **Engine:** LibreOffice headless. Licence MPL-2.0, no patent flag.
- **Options/settings:**
  - As source → CSV/TSV: see the shared **CSV/TSV export options** below
    (delimiter is fixed by the chosen target; encoding default **UTF-8**;
    **values-not-formulas** by default).
  - As source → PDF: see the shared **→ PDF options** below.
  - As target (← CSV/TSV): see **CSV/TSV import options** below.
  - No XLSX-specific user-facing switches in v1.
- **Lossy?** → CSV/TSV is lossy (formatting, multiple sheets, formulas-as-text,
  charts, colours dropped). → PDF is lossy (live → frozen). → XLS is lossy in the
  narrow sense that XLS caps at 65 536 rows / 256 columns and drops features XLS
  cannot represent (see XLS entry). → ODS is **practically lossless** for normal
  content (both are full spreadsheet models) but exotic Excel-only features may
  not round-trip. See §2.9.
- **Edge cases:** `.xlsm` (macro-enabled) is detected as XLSX-family; **macros
  are dropped** on every conversion (we never preserve or execute VBA — security
  and scope). Password/encrypted XLSX → cannot be opened headless without the
  password → **fail clearly** ("this file is password-protected") per §2.8, never
  a silent empty output. Very large sheets (≥ ~100 k rows) are handled but slow;
  progress per §1.11, "too big" pre-flight per §1.10.

### `XLS` — Legacy Excel (BIFF8)

- **Detection:** OLE2 Compound File Binary (`D0 CF 11 E0 A1 B1 1A E1`) containing
  a `Workbook`/`Book` stream. Extension `.xls`. The OLE2 magic is shared with
  legacy `.doc`/`.ppt`; the **stream name** disambiguates (Workbook ⇒ XLS).
- **Role:** both.
- **As source → targets:** **`XLSX` ★(default)**, `ODS`, `CSV`, `TSV`, `PDF`
  (LibreOffice). Default is **XLSX** — the everyday intent for an old `.xls` is
  "make it modern", and XLSX is the widely-compatible modern workbook.
- **As target ← sources:** `XLSX`, `ODS`, `CSV`, `TSV` (LibreOffice).
- **Engine:** LibreOffice headless. MPL-2.0, no patent flag.
- **Options/settings:** as source → CSV/TSV and → PDF use the shared option sets
  below. No XLS-specific switches.
- **Lossy?** As a *source* the read is faithful. As a *target*, XLS is the
  **only lossy workbook target**: hard limits **65 536 rows × 256 columns**,
  no >2003 features (sparklines, modern conditional formatting, long strings >32k
  chars truncated). Offered because `→ XLS` is a real "send it to someone on
  ancient Excel" ask, but flagged lossy. See §2.9.
- **Edge cases:** very old/odd BIFF variants LO cannot parse → fail clearly.
  Encrypted XLS → fail clearly (password-protected). Macros dropped.

### `ODS` — OpenDocument Spreadsheet

- **Detection:** ZIP container whose first stored entry is an uncompressed
  `mimetype` part with bytes
  `application/vnd.oasis.opendocument.spreadsheet`. Extension `.ods`. As with
  XLSX, detect by the inner mimetype, not the `.ods` name.
- **Role:** both.
- **As source → targets:** **`XLSX` ★(default)**, `XLS`, `CSV`, `TSV`, `PDF`
  (LibreOffice). Default **XLSX** — the common reason to convert an ODS is "share
  it with an Excel user".
- **As target ← sources:** `XLSX`, `XLS`, `CSV`, `TSV` (LibreOffice).
- **Engine:** LibreOffice headless (native format — highest-fidelity round-trip).
  MPL-2.0, no patent flag.
- **Options/settings:** shared CSV/TSV and → PDF sets below. No ODS-specific
  switches.
- **Lossy?** → CSV/TSV and → PDF lossy as above. → XLSX practically lossless for
  ordinary content; → XLS lossy (legacy limits). See §2.9.
- **Edge cases:** flat-XML `.fods` variant — detected as ODS-family, converts the
  same. Password-protected ODS → fail clearly. Macros dropped.

### `CSV` — Comma-Separated Values (plain delimited text)

- **Detection:** **no magic bytes** — CSV is plain text. Detection is
  content-based and probabilistic (SSOT *Recognize files by content*):
  1. Confirm the bytes are text (valid in a known encoding, no NUL runs / binary
     signatures) — otherwise it is not CSV.
  2. **Encoding sniff** (see Category-wide): BOM → exact; else UTF-8 validity
     check; else fall back to a single-byte codepage (**Windows-1252** default,
     see policy).
  3. **Delimiter sniff** across the first N (default **20**) non-empty lines:
     pick the candidate (`,` `;` `\t` `|`) giving the most **consistent** field
     count per line. A file whose dominant separator is a **tab** is classified
     as **TSV**, not CSV (grouping per §1.3 keys on the user-facing format).
  - Extension `.csv` is a hint only; a `.csv` that is really tab-separated is
    treated as TSV (content over name).
  - When detection is **ambiguous** (no consistent delimiter, or undecidable
    encoding) ConvertIA declines clearly rather than guessing a wrong split
    (SSOT *Recognize files by content*; §2.8).
- **Role:** both.
- **As source → targets:** **`XLSX` ★(default)** (LO), `XLS` (LO), `ODS` (LO),
  `TSV` (native). Default **XLSX** — turning a text dump into a real, formatted
  workbook is the dominant everyday want. *(→ PDF is out, see matrix note ².)*
- **As target ← sources:** `XLSX`, `XLS`, `ODS` (LibreOffice); `TSV` (native).
- **Engine:**
  - `CSV → XLSX/XLS/ODS`: **LibreOffice** (Calc import filter with explicit
    import `FilterOptions` carrying the **sniffed delimiter + encoding** so LO
    does not re-guess).
  - `CSV → TSV`: **native** Rust engine (re-encode to UTF-8 + swap delimiter,
    re-quoting per RFC-4180 where the tab or a quote appears in a field).
- **Options/settings (as source):**
  - **Text encoding (input)** — *Advanced*. Default **Auto-detect** (BOM →
    UTF-8 → Windows-1252 fallback). Override list: UTF-8, UTF-16 LE/BE,
    Windows-1252, ISO-8859-1, ISO-8859-15. The chosen/detected encoding is
    passed verbatim into LO's import `FilterOptions` (token 3) so the import is
    deterministic, not re-sniffed by LO.
  - **Delimiter (input)** — *Advanced*. Default **Auto-detect**. Override: comma
    / semicolon / tab / pipe / custom single char.
  - **Quoted fields are text** — *Advanced*, default **off** (let numbers be
    numbers). When **on**, quoted fields stay literal text — the fix for
    "`0123` lost its leading zero" / "`3/4` became a date" (LO import token
    "quoted field as text" = true; *Detect special numbers* = false).
- **Options/settings (as target):** see shared **CSV/TSV export options** below.
- **Lossy?** `CSV → XLSX/ODS` is **not** lossy (text in, richer container out —
  it only *adds* structure; the only risk is value mis-typing, which the
  "quoted fields are text" switch defends). `CSV → TSV` is **not** lossy (both
  are plain text; only the delimiter and possibly the encoding normalise to
  UTF-8). Producing CSV *from* a workbook is the lossy direction (recorded on the
  workbook entries), not these.
- **Edge cases:** mixed line endings (CRLF/LF/CR) normalised on read; embedded
  newlines inside quoted fields preserved (RFC-4180); a stray BOM is consumed,
  not emitted as a phantom first cell; ragged rows (uneven field counts) are
  kept as-is (short rows pad with empty cells into a workbook, never truncated);
  a leading `=`/`+`/`-`/`@` cell is **not** auto-executed as a formula on import
  (CSV-injection-safe: imported as text unless the user opts into formula
  evaluation, which v1 does **not** expose).

### `TSV` — Tab-Separated Values

- **Detection:** plain text whose sniffed dominant delimiter is the **tab**
  (`\t`, ASCII 9). Same encoding sniff as CSV. Extensions `.tsv`, `.tab`. A
  `.tsv` that is actually comma-separated is classified as CSV (content over
  name).
- **Role:** both.
- **As source → targets:** **`XLSX` ★(default)** (LO), `XLS` (LO), `ODS` (LO),
  `CSV` (native). Default **XLSX**, same rationale as CSV. *(→ PDF out.)*
- **As target ← sources:** `XLSX`, `XLS`, `ODS` (LibreOffice); `CSV` (native).
- **Engine:** identical split to CSV — LO for the workbook targets, native for
  `TSV → CSV`.
- **Options/settings:** same as CSV (encoding default Auto-detect/UTF-8,
  "quoted fields are text" default off) — the delimiter is fixed to tab on input
  by definition. As a target, see shared export options (TSV forces field
  separator = tab, ASCII 9).
- **Lossy?** Same as CSV: workbook/`→CSV` outputs from TSV are not lossy in the
  text-content sense. Producing TSV from a workbook is the lossy direction
  (recorded on workbook entries).
- **Edge cases:** because the tab is the separator, a field that itself contains
  a tab **must** be quoted on export — the native engine quotes per RFC-4180
  (this is the one real correctness trap of TSV and the engine handles it rather
  than silently splitting the field).

### `PDF` — (target only here; canonical entry in [documents.md](documents.md))

- **Not re-documented here.** Detection (`%PDF-`), the full as-target source
  list, and the PDF page options are owned by **[documents.md](documents.md)**.
- **What this file contributes:** the producer rows **`XLSX → PDF`**,
  **`XLS → PDF`**, **`ODS → PDF`** (all LibreOffice headless, MPL-2.0, no patent
  flag), and their spreadsheet-side options (orientation / fit-to-width / which
  sheets — see → PDF options below).
- **Lossy?** Yes — a live workbook becomes a fixed page layout: formulas freeze
  to their computed values, off-page columns may clip or scale, fonts may
  substitute. Recorded in §2.9 and surfaced as the passive inline note.

---

## Shared option sets (concrete values + defaults)

> These are the per-pair option lists §1.6 delegates to 04. Defaults are the
> **no-decision defaults** (SSOT *It just works by default*) — every one is
> chosen so the common path needs zero clicks.

### CSV / TSV **export** options (workbook → CSV/TSV)

Maps to LibreOffice's `Text - txt - csv (StarCalc)` export `FilterOptions`
token string (see §3.5 for exact assembly).

| Setting | Surface | Default | Values / notes |
|---------|---------|---------|----------------|
| **Field separator** | fixed by target | CSV → comma (ASCII **44**); TSV → tab (ASCII **9**) | Not user-chosen — the target *is* the delimiter. (Token 1.) |
| **Text delimiter (quote char)** | — | double-quote (ASCII **34**) | RFC-4180 quoting; fields containing the separator, a quote, or a newline are quoted. (Token 2.) |
| **Output encoding** | *Advanced* | **UTF-8** (token 3 = **76**) | Override: UTF-8, UTF-16, **Windows-1252** (token 3 = 1), ISO-8859-1/-15. |
| **Byte-order mark (BOM)** | *Advanced* | **off** for UTF-8 | On request only (token 14). UTF-8 without BOM is the portable default; a BOM is offered for users feeding Excel-on-Windows that mis-reads UTF-8. |
| **Cell content** | *Advanced* | **values as shown** (token 9 *Save cell contents as shown* = true; token 10 *Export cell formulae* = false) | The everyday default: a CSV of **results**, not `=A1+B1` strings. Optional **"export formulas instead of values"** flips tokens 9/10 — niche, *Advanced* only. |
| **Which sheet (multi-sheet)** | basic (only shown if >1 sheet) | **first/active sheet only** | See the multi-sheet decision below — this is the load-bearing **[OPEN]** detail. |
| Quote all text fields | *(not exposed v1)* | off | LO default; numbers unquoted. Adding it is a scope change. |

### CSV / TSV **import** options (CSV/TSV → workbook)

Maps to the import `FilterOptions` of the same filter.

| Setting | Surface | Default | Values / notes |
|---------|---------|---------|----------------|
| Input encoding | *Advanced* | **Auto-detect** → UTF-8 → Windows-1252 | Detected value passed as token 3 so LO does not re-sniff. |
| Input delimiter | *Advanced* | **Auto-detect** | Detected value passed as token 1. |
| Quoted fields as text | *Advanced* | **off** | On = leading-zero / date-string safe (token "quoted field as text" = true, *detect special numbers* = false). |
| First-row-is-header | *(not exposed v1)* | n/a | LO imports all rows as data; "header" is a downstream concern, not a conversion setting. |

### → PDF options (workbook → PDF)

The PDF *page* options (PDF/A, quality) belong to the canonical PDF entry; the
**spreadsheet-side** controls are:

| Setting | Surface | Default | Values / notes |
|---------|---------|---------|----------------|
| Sheets to print | *Advanced* | **all non-empty sheets** | Unlike CSV, PDF *can* hold multiple pages, so all populated sheets print (each sheet → its own page run). Empty sheets skipped. |
| Page orientation | *Advanced* | **inherit the document's print settings**, else portrait | The workbook's saved print setup wins; portrait is the fallback. |
| Fit wide sheets | *Advanced* | **fit-to-width (1 page wide)** | Default scales each sheet so columns are not clipped at the right margin — the common "why is half my table missing" PDF complaint. |

---

## Category-wide

### Per-source default targets (one-glance)

| Source | Pre-highlighted default | Why |
|--------|-------------------------|-----|
| **XLSX** | **CSV** | Most common everyday want from a finished workbook is "give me the data as text" for upload/import; CSV is the universal interchange. |
| **XLS**  | **XLSX** | "Modernise this old file" — XLSX is the widely-compatible current workbook. |
| **ODS**  | **XLSX** | "Share with an Excel user." |
| **CSV**  | **XLSX** | "Turn this text dump into a real, formatted spreadsheet." |
| **TSV**  | **XLSX** | Same as CSV. |

Each source has **exactly one** fixed default (per §1.5 / README convention).
The XLSX→CSV default is the one debatable call — see [OPEN] below.

### CSV / TSV encoding policy (SSOT *Content fidelity*)

- **On read (detect):** BOM first (UTF-8/UTF-16 BOMs are authoritative) → else
  try strict UTF-8 → else fall back to **Windows-1252** (the most common
  single-byte codepage for everyday Western files; a strict ISO-8859-1 fallback
  would mis-handle the `0x80–0x9F` range that real-world "Latin-1" files use for
  curly quotes, em-dashes, €). The detected encoding is shown in the collected
  summary line and is overridable in *Advanced* before convert.
- **On write:** **UTF-8 without BOM** by default — the portable, language-neutral
  choice that carries CJK/RTL/accented text intact (SSOT *Content fidelity*).
  Windows-1252 and a BOM-on toggle are *Advanced* escape hatches for users
  feeding tools that still expect them.
- **No silent transliteration:** characters that cannot be represented in a
  user-chosen non-Unicode output encoding are **not** dropped or `?`-replaced
  silently — the conversion is flagged lossy (data-loss note, §2.9) so the user
  is told. (UTF-8 output never hits this, which is why it is the default.)

### Delimiter detection policy

- Candidates probed: comma `,`, semicolon `;`, tab `\t`, pipe `|`. Winner = the
  one yielding the most **consistent** field-count across the sample (default
  first **20** non-empty lines), with a tie-break preferring the extension hint
  then comma.
- **Semicolon-CSV** (European Excel exports, where comma is the decimal point) is
  detected and handled — a file that is really `;`-separated is not mis-split on
  the literal `,` inside `1,50`.
- A **tab** winner reclassifies the file as **TSV** (drives batch grouping,
  §1.3, and the offered target set).
- The detected delimiter is surfaced in the collected summary and overridable in
  *Advanced*. Undetectable/ambiguous → decline clearly (§2.8), never a wrong
  split.

### Formulas vs values on export (the one that bites people)

- **Default = computed values**, *as shown* in the sheet (LO export token 9 true
  / token 10 false). A spreadsheet → CSV/TSV produces the **results**, which is
  what every normal "export my data" expectation is.
- The opposite ("write the formula text `=A1+B1`") is an *Advanced*-only toggle,
  off by default.
- → PDF likewise freezes formulas to their displayed values (a PDF is a picture
  of the computed sheet).
- The inverse risk on **import** (CSV cell `=cmd|...` being treated as a live
  formula — the CSV-injection / DDE class) is closed: imported delimited text is
  ingested as **data**, formula evaluation on import is **not** exposed in v1.

### Multi-sheet handling — `* → CSV/TSV` (load-bearing **[OPEN]**)

- **Hard constraint from the SSOT:** conversions are strictly
  **one-source → one-target**; **one-to-many fan-out is parked** (SSOT *Direction
  & shape rule*; README parked list). A workbook with N sheets therefore **cannot**
  legitimately produce N CSV files in v1 — that would be a fan-out.
- **Engine reality (verified):** LibreOffice headless `--convert-to csv` exports
  **only one sheet** (historically the *first/active* sheet); the CSV export
  filter's "export all sheets" token (`-1`) is unreliable across LO versions and,
  even where it works, produces multiple files — which we do not want. So the
  engine behaviour *aligns* with the one-target rule by default.
- **Therefore the v1 behaviour is: a multi-sheet workbook → CSV/TSV exports ONE
  sheet**, and ConvertIA tells the user this is happening (a passive note when
  the source has >1 sheet: *"only one sheet is exported to CSV"*) rather than
  silently dropping data. The **[OPEN]** detail is purely *which* sheet and how
  it is chosen:

  - **[OPEN] A — which single sheet.** Options: (a) the workbook's **active
    sheet** as saved (matches "what I was looking at when I saved", LO's natural
    headless behaviour); (b) always the **first physical sheet** (predictable,
    name-independent); (c) **let the user pick** the sheet from a dropdown when
    >1 sheet is detected (most honest, one extra optional click — does **not**
    violate "no required choices" if it defaults to the active/first sheet).
    *Leaning (c) with default = active sheet*, because silently exporting a sheet
    the user did not mean is a data-surprise the SSOT *Fail clearly* spirit
    dislikes. **Not resolved — needs a call.**

  - **[OPEN] B — single-sheet fast path.** When the workbook has exactly **one**
    sheet, no note and no picker — it just converts (the overwhelming common
    case). Only multi-sheet workbooks trigger the note/picker. *(This part is
    settled; only A is open.)*

- **What is NOT open:** the no-fan-out rule (multiple-CSV output stays parked),
  and the → PDF case (PDF *can* be multi-page, so → PDF keeps all sheets — no
  conflict).

### Metadata, styling & feature policy (what is dropped, by design)

- **Charts, images, pivot tables, conditional formatting, cell styles/colours,
  comments, named ranges, defined print areas** are preserved across
  workbook↔workbook conversions (XLSX↔ODS, etc.) to the extent both formats
  support them, and are **dropped** on `→ CSV/TSV` (text has no styling — this is
  inherent, covered by the lossy note) and **rasterised/frozen** on `→ PDF`.
- **Macros / VBA / scripts are always dropped** (never preserved, never
  executed) — scope and security.
- **Hidden rows/columns/sheets:** preserved workbook→workbook; on `→ CSV/TSV` of
  a single sheet, the data is taken as the sheet's used range (hidden columns
  included as data). *(Edge — noted for Phase-3 test corpus.)*

### Lossy disclosure

The lossy pairs in this category, each cross-referenced to the §2.9 string
catalog (this file records *which* pairs; §2.9 owns the exact note text):

| Pair(s) | Loss | §2.9 `LossyKind` |
|---------|------|-----------|
| `XLSX/XLS/ODS → CSV` and `→ TSV` | one sheet only; all formatting, formulas-as-text, charts, colours, multi-sheet structure dropped — values only | `sheet_to_delimited` |
| `XLSX/XLS/ODS → PDF` | live workbook → fixed page; formulas frozen, wide tables may scale/clip, fonts may substitute | `doc_pdf_reflow` *(shared office→PDF kind)* |
| `* → XLS` | legacy limits: 65 536 rows / 256 columns max; post-2003 features dropped | `xls_legacy_limits` |
| `CSV/TSV → workbook` with a non-Unicode chosen output encoding (rare) | un-representable characters would be lost — flagged, not silently dropped | `text_encoding_narrowed` |

`CSV ↔ TSV` and `CSV/TSV → workbook` (UTF-8) are **not lossy** and carry no note.

### Open items

- **[OPEN] XLSX default target = CSV vs XLSX-staying-put.** We set XLSX's default
  to **CSV** (most common "get the data out" want). Counter-argument: a user
  dropping an `.xlsx` may more often want `→ PDF` (share a frozen copy) than
  `→ CSV`. Both are defensible; **CSV chosen** for now as the data-centric
  everyday action, but this is the one source whose default is genuinely
  debatable and should be validated against the SSOT usability walkthrough.
- **[OPEN] Multi-sheet → CSV sheet selection** — see *Multi-sheet handling*
  above ([OPEN] A): active-sheet vs first-sheet vs user-picker. Leaning
  user-picker defaulting to active sheet. Needs a call before Phase 3.
- **[OPEN] Pipe-delimited (`.psv`) as a first-class TSV-sibling target?** —
  currently *not* offered as a target (only auto-*detected* as a CSV input
  variant). Likely stays out (niche), parked unless demand appears.
