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
- **As source → targets:** the full list of sensible targets (with the engine
  and any direction-specific notes).
- **As target ← sources:** which sources can produce it.
- **Engine(s):** primary + fallback, per platform; licence; patent flag.
- **Options/settings:** exposed switches (basic vs Advanced), **default value**
  (the no-decision default), valid ranges.
- **Lossy?:** whether conversions to/from are predictably lossy; the disclosure
  text.
- **Edge cases:** multi-page/animation/transparency/metadata/colour-profile,
  encoding, very large inputs, etc.

## Category file shape

Each category file contains: a short intro, a **source→target matrix table**
(rows = sources, cols = targets, cells = supported/engine/lossy), then one
templated entry per format, then category-wide edge cases & option defaults.

## Conventions
- A pair is `v1-required` unless the SSOT exceptions apply (patent per-platform;
  last-resort reliability demotion) — both are recorded inline where relevant.
  Patent dispositions reference the single matrix in §3.4 (not re-decided here).
- "Sensible" = passes the SSOT canonical inclusion test; degenerate/no-demand
  pairs are explicitly marked out.
- **Multi-category formats** (e.g. PDF, shared by Documents, Presentations &
  Spreadsheets) are documented **once** in a canonical home (PDF → `documents.md`).
  That canonical entry holds the **single complete** As-target enumeration —
  including every producer row from other categories (xlsx→pdf, pptx→pdf, …) — so
  the matrix is never assembled wrong; other files only *reference* it. The
  general "one detected type → de-duplicated union of targets" rule is owned by
  §1.5.
- **Options ownership:** the generic option-declaration model is owned by §1.6;
  the 04 files own the **concrete per-pair option lists and default values** (and
  are not restated in §1.6).
- **Lossy fields** record *which* pairs are lossy and **link to §2.9** (the string
  catalog) — they never restate the disclosure string.
- **Per-source default target:** every detected source has exactly **one** fixed,
  pre-highlighted default target — mark it in each source→target matrix and
  summarise all defaults in a one-glance table here.
- `cross-category.md` **intentionally departs** from this template: its entries
  are *operations* (extract-audio, to-GIF), not standalone formats.
