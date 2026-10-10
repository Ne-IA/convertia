# 04 — Format Matrix

> The per-category specification of **every format** ConvertIA handles and **every
> sensible conversion**, in **both directions**. Origin: SSOT *What It Converts*.
> One file per category; this README defines the shared template every entry uses.

## Categories (v1 — six)

| File | Category |
|------|----------|
| [images.md](images.md) | Images |
| [audio.md](audio.md) | Audio |
| [video.md](video.md) | Video |
| [documents.md](documents.md) | Documents |
| [spreadsheets.md](spreadsheets.md) | Spreadsheets |
| [presentations.md](presentations.md) | Presentations |
| [cross-category.md](cross-category.md) | Cross-category outputs (extract-audio, to-GIF) — closed set |

> Parked categories (not v1): archives, e-books, fonts, RAW/PSD — see SSOT
> *Future Ideas*.

## Per-format entry template

Each format in a category file is documented as:

### `<FORMAT>` (e.g. PNG)
- **Detection:** magic bytes / signature, ambiguity notes, extension(s).
- **Role:** source / target / both.
- **As source → targets:** the targets, with the engine and any direction-specific
  notes — commentary: the matrix is authoritative.
- **As target ← sources:** which sources can produce it — commentary, like the line
  above.
- **Engine(s):** primary + fallback, per platform; licence; patent flag — `none`, or a
  pointer to the codec's §3.4.2 fact line or §3.4.3 row.
- **Options/settings:** exposed switches (Basic vs Advanced), each as a §1.6 shape —
  `IntRange` (range, step, `Unit`), `Enum` (its choices), `Toggle`, `Size` or `Color` —
  with its **default value** (the no-decision default).
- **Lossy?:** whether conversions to/from are predictably lossy, by §2.9 kind (the
  category's `## Lossy kinds` table maps each pair).
- **Edge cases:** multi-page/animation/transparency/metadata/colour-profile,
  encoding, very large inputs, etc.

## Category file shape

Each category file contains, in this order: a short intro; `## Source → target matrix`
(the grammar below); for `audio.md` and `video.md`, a `## Decode inventory` section, the
one list of what the category decodes (§6.1.3): a table with one row per FFmpeg decoder
(codec | sources | configure component | `ffmpeg -decoders` name | `ffprobe` `codec_name` |
§3.4 home) and, in `video.md`, a text-subtitle table with the same columns except the §3.4
home; `## Lossy kinds`; one templated entry per format under `## Per-format entries`; and
`## Category-wide` (edge cases and option defaults that span formats). Other h2
sections (an intro, the engine binding) may sit between them.

`## Lossy kinds` is a table with the columns *Kind* | *Pairs* | *Layer* | *Condition*, one
row per §2.9.1 `LossyKind` the category fires: *Pairs* lists the (source, target) pairs
the kind is registered on, *Layer* is its §2.9.2 layer (`pair-static`, `source-fact` or
`per-item-runtime`), and *Condition* says when a registered kind fires (`—`: always).

## Matrix cell grammar `[DECIDED]`

One grammar serves the six category matrices and the cross-category.md operation matrix.

- **Header:** `| Source ↓ \ Target → | FMT | … |`. A row label is the source format in
  bold; a row or column label may carry a footnote mark.
- **Cell** — exactly one of:
  - `·` — not offered: the pair fails the SSOT inclusion test (no everyday demand, or
    degenerate);
  - `—` — the same-format diagonal, not offered (§1.5 rule 5); the category's diagonal
    note says why;
  - `out` + a footnote — parked: out of v1 by the SSOT (the direction & shape rule,
    *Future Ideas*); the footnote names the reason;
  - `✓` + optional flags + an optional engine tag + an optional footnote — offered.
- **Flags on a `✓` cell**, written in this order:
  - `★` — the row's default target (§1.5 rule 3), exactly one in each category-matrix
    row that offers a target;
  - `~` — lossy: a `pair-static` or `per-item-runtime` row of the category's
    `## Lossy kinds` table lists the pair, so a note shows at target choice (§2.9.2); a
    `source-fact` kind alone never sets it;
  - `R` — video only, always after `~`: re-encoding is the pair's worst case, not its
    rule — a lossless remux is common, and the note is the `video_reencode`
    before-convert line. A video `~` cell without `R` always re-encodes.
- **Engine tag**, after the flags: the §3.2 registry engine that serves the pair — `img`
  (the image worker, §3.5.5), `ff` (FFmpeg, §3.5.1), `lo` (LibreOffice, §3.5.2), `pp`
  (poppler, §3.5.3), `pd` (pandoc, §3.5.4) or `csv` (the in-core CSV/TSV engine,
  §3.5.6). A category served by one engine names it above its matrix, and its cells carry
  no tag.
- **Footnote:** a superscript mark (`¹`, `²`, …), explained below the matrix.
- **Video's normalize diagonal** is offered (video.md *Same-container*, §1.5 rule 5), so
  its diagonal cells are `✓` cells; every other diagonal cell is `—`.
- **Operation matrix** (cross-category.md): the rows are the video sources, and each
  column is one §0.6 `TargetId` — `Extract audio → MP3 ★`, `→ WAV`, `→ FLAC`, `→ M4A`,
  `→ OGG` and `To animated GIF`. Its cells use the same grammar and carry no `★`: a
  video row's default target is its video.md `★`. The `★` in the `Extract audio → MP3`
  header marks MP3 as the everyday extract target (cross-category.md `[XCAT-A]`).

## Conventions
- A pair is `v1-required` unless the SSOT exceptions apply (patent per-platform;
  last-resort reliability demotion) — both are recorded inline where relevant.
  Patent dispositions reference the single matrix in §3.4 (not re-decided here).
- "Sensible" = passes the SSOT canonical inclusion test; a pair that fails it is `·`
  in the matrix, a parked one `out` (*Matrix cell grammar*).
- **Multi-category formats** (e.g. PDF, shared by Documents, Presentations &
  Spreadsheets) are documented **once** in a canonical home (PDF → `documents.md`).
  That canonical entry holds the **single complete** As-target enumeration —
  including every producer row from other categories (xlsx→pdf, pptx→pdf, …) — so
  the matrix is never assembled wrong; other files only *reference* it. The
  general "one detected type → de-duplicated union of targets" rule is owned by
  §1.5.
- **Options ownership:** the generic option-declaration model is owned by §1.6;
  the 04 files own the **concrete per-pair option lists and default values** (and
  are not restated in §1.6). A batch has one `OptionValues`, so no option takes
  per-file values (§1.6).
- **Lossy fields** record *which* pairs are lossy and **link to §2.9** (the string
  catalog) — they never restate the disclosure string. The category's
  `## Lossy kinds` table is the pair → kind map.
- **Per-source default target:** every detected source has exactly **one** fixed,
  pre-highlighted default target — the `★` in its matrix row; each category file's
  one-glance defaults table gives the reason per source.
- `cross-category.md` **intentionally departs** from the entry template: its entries
  are *operations* (extract-audio, to-GIF), not standalone formats; its operation
  matrix follows the grammar above.
