# 04 — Formats: Presentations

> Formats (SSOT *What It Converts* → Presentations): **PPTX, PPT, ODP, PDF**.
> Follows the per-format template in [README](README.md).
>
> **Scope of this file.** Presentations is a small, single-engine category: the
> three editable office formats (PPTX, PPT, ODP) and the one shared output (PDF).
> The only sensible everyday conversion is **presentation → PDF** (share / print /
> "send a copy that opens anywhere"), plus **office ↔ office** re-encoding between
> the three editable formats. **PDF is a multi-category format and is documented
> canonically in [`documents.md`](documents.md)** — this file only adds the
> `PPTX/PPT/ODP → PDF` *producer rows* to PDF's As-target enumeration and
> **references** the canonical PDF entry for detection, options, and the rest of
> PDF's targets. Per the README single-owner rule, PDF is **not** re-specified
> here.
>
> **Reverse direction is out (SSOT direction & shape rule).** `PDF → PPTX/PDF →
> ODP` is a *reconstructive* conversion (re-deriving editable slides from a flat
> page format) — explicitly out of v1 per the SSOT (the same class as
> `pdf→docx`). PDF therefore appears **only as a target** of this category, never
> as a source of a presentation. **Slide → image fan-out** (one PNG/JPG per slide)
> is a one-to-many conversion and is **parked** (SSOT *Future Ideas*); see
> [OPEN/Parked] at the foot of this file.

## Engine

A **single engine** covers the entire category: **LibreOffice (headless /
`soffice --headless`)**, driven through its UNO conversion path. LibreOffice
imports PPTX, PPT, ODP and exports to PPTX, PPT, ODP and PDF, so **every pair in
this category is satisfied by one engine with no chaining** (satisfies §3.2
single-engine-per-pair). The same engine instance/profile is shared with
`documents.md` and `spreadsheets.md` (it is the Office workhorse).

| Aspect | Value |
|--------|-------|
| Engine | LibreOffice headless (Impress import/export + PDF export filter) |
| Invocation | `soffice --headless --convert-to <ext>:<FilterName>[:<FilterData JSON>] --outdir <tmp> <input>` (UNO under the hood) |
| Licence | **MPL-2.0** (LibreOffice is weak-copyleft / file-level). Ships as a **separate, independently-invoked binary** (sidecar), never linked into the MIT core — consistent with the SSOT copyleft-isolation policy and §3.6. |
| Patent flag | **None** for this category. PPTX/PPT/ODP/PDF carry no patent gate (contrast HEIC/AAC in §3.4). Available on all three platforms. |
| Isolation | Untrusted office files are parsed by LibreOffice inside the §2.12 decoder-isolation boundary; a crash/hang fails the one item (§2.8) and never wedges the app. |
| Per-engine args | Concrete `soffice` argument construction, profile/`-env:UserInstallation` handling, the one-document-per-invocation rule, and exit-code/stderr quirks live in §3.5 (not restated here). |

> **Engine notes that matter for fidelity.** LibreOffice is a *re-rendering*
> converter: it loads the presentation into Impress's own document model and
> re-lays-it-out, then writes the target. This is why PPT↔PPTX↔ODP and →PDF can
> shift fonts, spacing, and effects (see Lossy, below). There is no
> "pass-through"/lossless path for office formats — every conversion round-trips
> through Impress's model.

## Source → target matrix

Rows = source (detected) format, cols = target. Legend:
**✓** supported · **✓★** supported **and the pre-highlighted DEFAULT** for that
source · **✓~** supported but **predictably lossy** (see §2.9) · **★~** default
**and** lossy · **—** not offered · **out** degenerate / reverse / parked (with
reason). Engine short-name **LO** = LibreOffice headless.

| Source ↓ \ Target → | PDF | PPTX | PPT | ODP |
|---|---|---|---|---|
| **PPTX** | ★~ LO | — *(same format)* | ✓~ LO | ✓~ LO |
| **PPT**  | ★~ LO | ✓ LO | — *(same format)* | ✓~ LO |
| **ODP**  | ★~ LO | ✓~ LO | ✓~ LO | — *(same format)* |
| **PDF**  | — *(canonical home: `documents.md`)* | out — reverse/reconstructive (parked) | out — reverse/reconstructive (parked) | out — reverse/reconstructive (parked) |

Notes on the cells:

- **Default = PDF for all three sources.** PDF is the obvious everyday "share a
  copy that opens anywhere / print / submit" output and is widely compatible
  (SSOT default tie-breaker favours the widely-compatible target). Marked `★`.
- **→ PDF is flagged lossy (`~`)** for every source: fonts may substitute,
  animations/transitions are flattened, and editability is lost. This is
  *predictable* loss → an inline note at target choice (§2.9). It is still the
  right default; the note is passive, not a gate (SSOT Principle 7 / 8).
- **office ↔ office** re-encodings between PPTX/PPT/ODP are offered because a
  normal person genuinely wants them ("I have a `.odp`, my school needs
  `.pptx`"; "open this old `.ppt` in modern PowerPoint as `.pptx`"). They are
  **lossy whenever crossing the MS↔ODF boundary** (`✓~`): ODP→PPTX, ODP→PPT,
  PPTX→ODP, PPT→ODP all round-trip through Impress's model and can drop/approximate
  features. **The two within-MS-family directions are NOT symmetric `[OPEN-1] resolved`:**
  **`PPT→PPTX` (modernizing to a richer format) stays plain `✓`** — the newer format can
  hold everything the legacy one did. **`PPTX→PPT` (downgrading to legacy BIFF8 /
  PowerPoint-97) is `✓~` lossy** — PPT **structurally cannot represent** SmartArt, modern
  charts, or newer transitions (e.g. Morph), so these are simplified/dropped; it carries
  the §2.9 **`pptx_to_ppt_legacy`** note. (See Lossy disclosure.)
- **Same-format cells are `—`** (PPTX→PPTX etc.). A presentation→same-format
  "conversion" has no everyday demand and is degenerate; it is **not** offered as
  a target (unlike images, there is no "re-compress" use case here). The no-harm
  re-encode-in-place machinery (SSOT Principle 5) is irrelevant because the pair
  is never offered.
- **PDF row is all `—`/out.** PDF→PDF lives in `documents.md`; PDF→PPTX/PPT/ODP
  are reverse/reconstructive and parked (SSOT direction rule).

## Per-format entries

### `PPTX` (PowerPoint 2007–365 / Office Open XML Presentation)

- **Detection:** PPTX is a **ZIP (OPC) container** → magic bytes `50 4B 03 04`
  (`PK\x03\x04`). It is **not** distinguishable from DOCX/XLSX/ODP/generic ZIP by
  the leading bytes alone — content sniffing **must** look inside: a PPTX has
  `[Content_Types].xml` declaring
  `application/vnd.openxmlformats-officedocument.presentationml.*` and a
  `ppt/presentation.xml` part. Detection keys on those, **not** the `.pptx`
  extension (SSOT *Recognize files by content*); a `.pptx` that is really a DOCX
  is grouped as DOCX, and a mis-named `.zip` that is really a PPTX is offered the
  PPTX targets. MIME:
  `application/vnd.openxmlformats-officedocument.presentationml.presentation`.
  Related extensions handled as PPTX-class on import: `.pptm` (macro-enabled —
  macros are **not** executed; VBA is dropped on any re-export), `.ppsx`/`.pps`
  (slideshow autoplay variant — treated as a presentation source).
- **Role:** **both** (source and target).
- **As source → targets:**
  - `→ PDF` — **LO**, `impress_pdf_Export`. **DEFAULT (★)**. Lossy (`~`).
  - `→ PPT` — **LO**, `MS PowerPoint 97`. **Lossy (`✓~`):** legacy BIFF/PPT97 cannot
    store SmartArt, modern charts, or Morph transitions → those are flattened/dropped on
    down-save → §2.9 `pptx_to_ppt_legacy`. (Within the MS family, but NOT lossless — the
    matrix cell is `✓~`, the §2.9 Lossy section lists it, and the mandatory lossy
    disclosure applies. The earlier "plain ✓" was wrong.)
  - `→ ODP` — **LO**, `impress8`. Crosses MS→ODF boundary → lossy (`✓~`).
  - `→ PPTX` — **not offered** (same format, degenerate).
- **As target ← sources:** `PPT → PPTX` (LO, MS-family), `ODP → PPTX` (LO, lossy).
  *(No `PDF → PPTX` — reverse, parked.)*
- **Engine(s):** LibreOffice headless, all platforms. Import filter
  `Impress MS PowerPoint 2007 XML`. Licence MPL-2.0, sidecar. **No patent flag.**
- **Options/settings:**
  - As **→ PDF target**: the PDF FilterData options apply (see PDF options block
    in the Category-wide section). Default profile = no options set (engine
    defaults), exposed switch = **"Include speaker notes pages"** (off).
  - As **→ PPT / → ODP target**: **no exposed options** — straight format
    re-encode at engine default. (No quality/compression knob is meaningful for
    office→office; SSOT *It just works by default*.)
- **Lossy?:** As source to PDF — yes (§2.9 `slides_to_pdf_flatten`); to ODP — yes,
  crossing MS→ODF (§2.9 `office_roundtrip_approx`). **To PPT — yes, `✓~`** ([OPEN-1]
  resolved): downgrading to legacy BIFF8 loses SmartArt / modern charts / Morph that PPT
  cannot store → §2.9 **`pptx_to_ppt_legacy`** note.
- **Edge cases:** **embedded media** (video/audio in slides) — *not* embedded in
  the PDF; a poster/first-frame is rendered, the media itself is dropped (a known
  LibreOffice limitation; note this is part of the →PDF lossy disclosure).
  **Embedded fonts** inside the PPTX are used if present; otherwise font
  substitution applies (see Category-wide *Font handling*). **OLE objects**
  (embedded Excel charts etc.) render as their last-saved picture.
  **Animations/transitions/triggers** are flattened to the slide's final state in
  PDF. **Very large decks** (hundreds of slides, huge images) — handled, but
  drive the §1.10 size/time pre-flight; a single huge embedded image can dominate
  output size. **Corrupt/partial OPC zip** → fail clearly (§2.8), batch continues.

### `PPT` (PowerPoint 97–2003 / legacy binary `MS-PPT`)

- **Detection:** Legacy **OLE2 Compound File Binary** container → magic bytes
  `D0 CF 11 E0 A1 B1 1A E1`. This signature is **shared** with legacy DOC and XLS
  (all are CFB); detection **must** read the CFB directory and identify the
  PowerPoint document stream (`PowerPoint Document` / the
  `0x64656E796D6F....` PPT-specific streams) to distinguish PPT from DOC/XLS —
  the `.ppt` extension is not trusted. MIME: `application/vnd.ms-powerpoint`.
  Autoplay variant `.pps` (PowerPoint 97 AutoPlay) is treated as a PPT-class
  source.
- **Role:** **both** (source and target).
- **As source → targets:**
  - `→ PDF` — **LO**, `impress_pdf_Export`. **DEFAULT (★)**. Lossy (`~`).
  - `→ PPTX` — **LO**, `Impress MS PowerPoint 2007 XML`. Within MS family;
    *modernizes* the legacy deck (plain ✓). High everyday demand ("open my old
    `.ppt` in current PowerPoint").
  - `→ ODP` — **LO**, `impress8`. Crosses MS→ODF → lossy (`✓~`).
  - `→ PPT` — **not offered** (same format).
- **As target ← sources:** `PPTX → PPT` (LO, MS-family — "save back to old
  format"), `ODP → PPT` (LO, lossy). *(No `PDF → PPT`.)*
- **Engine(s):** LibreOffice headless, all platforms. Import filter for legacy
  binary PPT. Export filter `MS PowerPoint 97`. Licence MPL-2.0, sidecar.
  **No patent flag.**
- **Options/settings:** identical model to PPTX — PDF options only as a →PDF
  target; no options for office→office.
- **Lossy?:** As source to PDF — yes (§2.9 `slides_to_pdf_flatten`); to ODP — yes,
  crossing MS→ODF (§2.9 `office_roundtrip_approx`). **To PPTX — NOT lossy, plain `✓`**
  ([OPEN-1] resolved): this is the *modernizing* direction — the richer PPTX can hold
  everything the legacy PPT did, so no §2.9 note (unlike the reverse PPTX→PPT downgrade).
- **Edge cases:** Legacy binary PPT can carry **VBA macros** — never executed,
  dropped on re-export. **Older/rare PPT features** (some legacy effects,
  WordArt) may render approximately. **Embedded OLE/media** behave as for PPTX.
  CFB ambiguity (PPT vs DOC vs XLS) is the headline detection risk — covered by
  the stream-level check above.

### `ODP` (OpenDocument Presentation)

- **Detection:** ODF is a **ZIP container** with magic `50 4B 03 04` (`PK\x03\x04`),
  but the **first stored entry is an uncompressed `mimetype` member** whose bytes
  are `application/vnd.oasis.opendocument.presentation` — this is the reliable
  ODP discriminator and the canonical ODF detection trick (the `mimetype` part is
  stored first, uncompressed, by spec). Detection keys on that, **not** the `.odp`
  extension, and it cleanly separates ODP from PPTX/ODT/ODS/plain-zip despite the
  shared `PK` prefix. Template variant `.otp`
  (`...opendocument.presentation-template`) is treated as an ODP-class source.
  MIME: `application/vnd.oasis.opendocument.presentation`.
- **Role:** **both** (source and target).
- **As source → targets:**
  - `→ PDF` — **LO**, `impress_pdf_Export`. **DEFAULT (★)**. Lossy (`~`).
  - `→ PPTX` — **LO**, `Impress MS PowerPoint 2007 XML`. Crosses ODF→MS →
    lossy (`✓~`). High demand ("my LibreOffice deck, they need PowerPoint").
  - `→ PPT` — **LO**, `MS PowerPoint 97`. Crosses ODF→legacy-MS → lossy (`✓~`).
    Lower demand than PPTX but a normal person with an old PowerPoint may want it.
  - `→ ODP` — **not offered** (same format).
- **As target ← sources:** `PPTX → ODP` (LO, lossy), `PPT → ODP` (LO, lossy).
  *(No `PDF → ODP`.)*
- **Engine(s):** LibreOffice headless, all platforms. **Native format** — ODP is
  LibreOffice's own, so *import* is the highest-fidelity of the three. Export
  filter `impress8`. Licence MPL-2.0, sidecar. **No patent flag.**
- **Options/settings:** PDF options only as →PDF target; none for office→office.
- **Lossy?:** As source to PDF — yes (§2.9 `slides_to_pdf_flatten`). To PPTX/PPT —
  yes, crossing ODF→MS (§2.9 `office_roundtrip_approx`). ODP is the
  **most-faithfully-rendered source** to PDF because it is native, but PDF still
  flattens animations/effects.
- **Edge cases:** ODP→PPTX/PPT can lose ODF-only features (certain custom shapes,
  presentation-specific styles, some transition types absent in the MS schema).
  Embedded media/fonts/OLE behave as for PPTX. Otherwise as the common edge cases
  below.

### `PDF` — see [`documents.md`](documents.md)

PDF is documented **once**, canonically, in `documents.md` (detection signature
`%PDF-` / `25 50 44 46`, full As-target union, default, all PDF options and lossy
rows). **This category contributes the following producer rows** to PDF's
As-target enumeration in `documents.md` (recorded here for cross-check; the
authoritative list lives there):

| Producer (this category) | Engine | Filter | Lossy |
|---|---|---|---|
| `PPTX → PDF` | LO | `impress_pdf_Export` | yes (§2.9 `slides_to_pdf_flatten`) |
| `PPT → PDF`  | LO | `impress_pdf_Export` | yes (§2.9 `slides_to_pdf_flatten`) |
| `ODP → PDF`  | LO | `impress_pdf_Export` | yes (§2.9 `slides_to_pdf_flatten`) |

PDF is **not** a presentation *source* (reverse direction out of v1).

## Category-wide

### Per-source default summary (one-glance)

| Source | Pre-highlighted DEFAULT target | Why |
|--------|-------------------------------|-----|
| PPTX | **PDF** | Share/print/submit a copy that opens anywhere; widely compatible. |
| PPT  | **PDF** | Same; (modernizing to PPTX is the strong secondary target). |
| ODP  | **PDF** | Same; (ODP→PPTX is the strong secondary "they need PowerPoint" target). |

(PDF has no default *from* this category — it is target-only here; its own default
lives in `documents.md`.)

### Options & defaults — the **→ PDF** export (the only pair with exposed options)

All three sources share the **same** PDF-export option set (LibreOffice
`impress_pdf_Export` FilterData). ConvertIA exposes **one** basic switch and keeps
the rest at engine defaults (SSOT *It just works by default* — no required
choices; adding a setting is a scope change, §1.6). FilterData is passed on the
`soffice --convert-to` command line as JSON, e.g.
`pdf:impress_pdf_Export:{"ExportNotesPages":{"type":"boolean","value":"true"}}`.

| Option (LO FilterData) | UI tier | ConvertIA default | Engine default | Range / values | Notes |
|---|---|---|---|---|---|
| **ExportNotesPages** | **Basic** ("Include speaker-notes pages") | **off** (false) | false | bool | When on, appends each slide's notes page after the slides. The one switch a normal user actually asks for. |
| `Quality` (JPEG) | Advanced | 90 | 90 | 1–100 | Image compression quality inside the PDF. Left at engine default. |
| `ReduceImageResolution` | Advanced | false | false | bool | If on, downsamples images to `MaxImageResolution`. |
| `MaxImageResolution` | Advanced | 300 | 300 | 75/150/300/600/1200 DPI | Only effective with `ReduceImageResolution=true`. |
| `UseLosslessCompression` | Advanced | false | false | bool | PNG-style lossless instead of JPEG; larger files. |
| `SelectPdfVersion` | Advanced | 0 (PDF 1.7) | 0 | 0=PDF 1.7 (default, no restriction), 1=PDF/A-1b, 2=PDF/A-2b, 3=PDF/A-3b, 15=PDF 1.5, 16=PDF 1.6, 17=PDF 1.7 (per official LO `pdf_params`; 15/16/17 are plain PDF versions, NOT PDF/A — PDF/A is 1/2/3) | PDF/A only if a user needs archival (`1`/`2`/`3`); not default. |
| `ExportBookmarks` | (not exposed) | true | true | bool | Slide titles → PDF outline; harmless default-on. |
| `UseTaggedPDF` | (not exposed) | false | false | bool | Accessibility tags; **left at the Impress engine default `false`** — Impress tagged-PDF support is limited and yields noisy/low-value tag trees from slide layouts. **Deliberately unlike documents.md, where Writer sets `UseTaggedPDF=true`** (Writer emits well-structured heading/paragraph tags). The asymmetry is intentional, not a harmonisation gap. |
| `EmbedStandardFonts` | (not exposed) | false | false | bool | Embeds the 14 base PDF fonts; off by default. |

> **Default rationale.** ConvertIA ships the **bare engine defaults plus one
> Basic switch**. No option is *required*; the two-click `drop → PDF → convert`
> path uses zero settings. The "speaker-notes" switch is the only one with broad
> everyday demand ("export the deck **with** my notes for the printout"). The
> rest are genuine Advanced refinements and stay folded away (§1.6 basic-vs-
> Advanced).
>
> **office → office has no exposed options at all** — it is a straight format
> re-encode at the engine default (there is no meaningful quality/compression knob
> for PPTX/PPT/ODP interchange). This is intentional, not a gap.

### Lossy disclosure (links to §2.9; strings live there, not here)

Predictable, disclosed loss in this category, by pair (the §2.9 catalog owns the
exact note strings; this table only records *which* pairs are lossy and *what
class* of loss):

| Pair | §2.9 `LossyKind` | What is lost / changed |
|---|---|---|
| `PPTX/PPT/ODP → PDF` | `slides_to_pdf_flatten` | Editability lost; **animations/transitions/triggers flattened** to final slide state; **embedded video/audio dropped** (poster only); **fonts substituted** if not embedded → reflow/clipping; speaker notes omitted unless the notes switch is on. |
| `ODP → PPTX/PPT`, `PPTX/PPT → ODP` | `office_roundtrip_approx` | Cross-model (ODF↔MS) round-trip: ODF-only shapes/styles/transitions and MS-only effects (some SmartArt/WordArt/transition types) approximated or dropped to fit the other schema; minor layout shift. |
| `PPT → PPTX` (modernizing) | *(none — resolved [OPEN-1])* | Within-MS-family re-render to a *richer* format; the newer format holds everything the legacy one did → **not** flagged. |
| `PPTX → PPT` (downgrade to legacy) | `pptx_to_ppt_legacy` | Downgrade to BIFF8/PowerPoint-97: **SmartArt, modern charts, and newer transitions (e.g. Morph) cannot be stored** in the legacy format → simplified or dropped. A genuine content-faithfulness loss → §2.9 note. |

All →PDF pairs surface a **single passive inline note** (`slides_to_pdf_flatten`)
at the moment PDF is the chosen target (SSOT Principle 7: calm, non-blocking, not a
per-conversion nag).

> **[OPEN-1] resolved — the two MS-family directions are asymmetric.**
> **`PPT → PPTX` (modernizing)** does **not** get a §2.9 note: it goes to a *richer*
> format that holds everything the legacy one did — any drift is incidental, not the
> *predictable, content-faithfulness* loss §2.9 is scoped to (§2.9.2).
> **`PPTX → PPT` (downgrade to legacy BIFF8) DOES get the `pptx_to_ppt_legacy` §2.9 note:**
> PPT **structurally cannot represent** SmartArt, modern charts, or newer transitions
> (Morph), so they are simplified or dropped — a real content-faithfulness loss, disclosed
> per SSOT *fail/disclose clearly*. (The earlier blanket "within-MS-family is not-lossy"
> resolution was too broad downward and is corrected here.) The cross-model
> `office_roundtrip_approx` note still covers the ODF↔MS direction.

### Font handling (the dominant fidelity factor)

Slide fidelity hinges on fonts, and a converter on a *user's* machine cannot
assume PowerPoint's fonts are installed. ConvertIA's policy:

1. **Embedded fonts win.** If the source embeds its fonts (PPTX/ODP can), the
   bundled LibreOffice uses them — best fidelity, no substitution.
2. **Bundle a sensible base font set** with the LibreOffice sidecar so common
   decks render acceptably offline (the bundled-font inventory and whether to
   ship metric-compatible substitutes — e.g. Liberation/Carlito/Caladea for
   Arial/Calibri/Cambria — is owned by §3.x bundling; **[OPEN-2] `[DECIDED]`** below —
   §3.9.3 baseline, only CJK breadth `[DEFER: size]`). Metric-compatible substitutes keep
   line breaks ⇒ much less reflow than arbitrary fallback.
3. **No runtime font download** (SSOT offline floor) — missing fonts are
   substituted from the bundle, never fetched.
4. Font substitution is part of the →PDF / cross-family lossy disclosure (it is
   the single biggest cause of "my slide moved").

### Encoding / language / colour

- **Text content fidelity** (CJK, RTL/Arabic/Hebrew, mixed scripts) comes through
  intact (SSOT *Content fidelity*) **provided a glyph-bearing font is available**
  — this re-emphasises the bundled-font set ([OPEN-2] `[DECIDED]`) must cover at least
  Latin + common CJK/RTL coverage, or those slides render with `.notdef` boxes. This is
  the same constraint as `documents.md`.
- **Colour:** slides are RGB; PDF export keeps RGB. No CMYK/colour-management
  decisions are exposed (out of scope for everyday presentations).

### Edge cases common to the whole category

- **Container-format detection collisions** are the headline risk (all covered in
  per-format Detection): PPTX/ODP both start `PK` (ZIP); PPT shares the CFB magic
  with DOC/XLS. Detection **must** look inside (OPC content-types / ODF `mimetype`
  / CFB streams), never trust the extension (SSOT Principle 6). A `.zip` that is
  really a deck converts; a deck that is really a `.docx` groups as a document.
- **Macro-enabled / autoplay variants** (`.pptm`, `.ppsx`, `.pps`, `.potx`,
  `.otp`) are accepted as their base presentation type; **macros/VBA are never
  executed and are dropped** on any re-export (security + the SSOT
  "open arbitrary, possibly malicious files" posture, §2.12).
- **Password-protected / encrypted** presentations: ConvertIA does **not** prompt
  for a password in v1 — an encrypted deck **fails clearly** (§2.8: "this file is
  password-protected and can't be converted"), batch continues. (Same stance as
  encrypted PDF in `documents.md`.)
- **Empty / 0-byte / corrupt-container** files fail clearly per §2.8; the rest of
  a same-format batch keeps going.
- **Very large decks** feed the §1.10 resource pre-flight (output-size/time
  estimate, "too big" fast-fail). Conversion runs through the §1.7 engine
  lifecycle (cancellable progress) and the §2.12 isolation wrapper; LibreOffice's
  one-document-per-headless-invocation behaviour and profile handling are §3.5's
  concern.
- **No-harm / atomicity** (SSOT Principle 5): identical to every other category —
  source never touched, write-to-temp + atomic rename, no-clobber numbering,
  per-location divert (§2.1/§2.2/§2.7). Nothing presentation-specific overrides it.

### Decisions / Parked (resolved)

- **[OPEN-1] — RESOLVED (asymmetric): `ppt→pptx` is NOT a disclosed loss; `pptx→ppt` IS.**
  The two within-MS-family directions differ. **`ppt→pptx` (modernizing)** goes to a
  *richer* format that holds everything the legacy one did → incidental drift only, **no
  §2.9 note**. **`pptx→ppt` (downgrade to legacy BIFF8/PowerPoint-97)** **structurally
  loses** SmartArt, modern charts, and newer transitions (Morph) that PPT cannot store →
  the new **`pptx_to_ppt_legacy`** §2.9 note fires. The cross-model
  `office_roundtrip_approx` note still covers the ODF↔MS direction. (Corrects the earlier
  too-broad "within-MS is not-lossy" reading; no longer open, retained for traceability.)
- **[OPEN-2] — Bundled font set for fidelity. `[DECIDED]` (resolved centrally).** The
  font set shipped with the LibreOffice sidecar is the §3.9.3 baseline
  (Liberation+Carlito+Caladea metric-compatible MS substitutes + a curated Noto CJK/RTL
  subset), *shared* with `documents.md` and `spreadsheets.md` (same engine, same font
  dependence) and owned once in §3.x bundling. Only the **CJK breadth** remains
  `[DEFER: size]` against the binary-size budget (§3.9). Recorded here because fonts are
  the dominant fidelity lever for slides.
- **[OPEN-3] — Notes-pages switch. `[DECIDED]` → `ExportNotesPages=true`.** The single
  Basic switch ("Include speaker-notes pages") maps to **`ExportNotesPages=true`** (notes
  **pages**, the full-page layout) — NOT `ExportNotes=true` (notes as PDF annotations) —
  because that is what users mean by "export with my notes". The exact UI-string final
  form is a §5 concern (not a §04 design call), not an open question.
- **[PARKED] — Slide → image fan-out** (one PNG/JPG per slide, into a named
  folder). This is a **one-to-many** conversion → out of v1 by the SSOT direction
  rule, parked under *Future Ideas (one-to-many fan-out)*. LibreOffice **can**
  produce it (`impress_png_Export` / `impress_jpg_Export` per page, or via Draw),
  so it is a clean post-v1 add — noted so the capability isn't lost.
- **[PARKED] — PDF → PPTX/ODP** (reverse/reconstructive) — out of v1 (SSOT
  direction rule), parked with the general reverse-conversion family.
