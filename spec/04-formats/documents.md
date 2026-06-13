# 04 — Formats: Documents

> Formats (SSOT): PDF, DOCX, DOC, ODT, RTF, TXT, MD, HTML.
> Follows the per-format template in [README](README.md).

## Source → target matrix
_(rows = source, cols = target; both directions — fill.)_ **PDF is documented
canonically in this file** (its detection + full target union + default);
`presentations.md` references this PDF entry and only adds PPTX/PPT/ODP→PDF.

## Engine(s)
- Office formats: LibreOffice (headless); PDF↔text: poppler/pdftotext,
  Ghostscript; MD/HTML: pandoc. Licence + isolation notes. _(fill)_

## Per-format entries
_(PDF, DOCX, DOC, ODT, RTF, TXT, MD, HTML — one each: detection, targets both
ways, options + defaults, lossy [layout/reflow/fonts], edge cases: fonts, images,
encoding, password-protected PDF (out), forms) — **fill**_

## Category-wide
- Lossy disclosure (pdf→txt, docx→pdf reflow); font embedding/substitution;
  encoding; the multi-page→images fan-out is parked. _(fill)_
