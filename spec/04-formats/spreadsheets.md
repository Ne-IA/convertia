# 04 — Formats: Spreadsheets

> Formats (SSOT): XLSX, XLS, ODS, CSV, TSV.
> Follows the per-format template in [README](README.md).

## Source → target matrix
_(rows = source, cols = target; both directions — fill)_

## Engine(s)
- LibreOffice (headless) for spreadsheet formats + →PDF; CSV/TSV handling
  (encoding/delimiter detection). _(fill)_

## Per-format entries
_(XLSX, XLS, ODS, CSV, TSV — one each: detection, targets both ways, options +
defaults, lossy, edge cases: multi-sheet → CSV (one sheet? which? — resolve),
encoding/delimiter (UTF-8/Windows-1252), formulas vs values, big sheets) — **fill**_

## Category-wide
- CSV encoding/delimiter policy (SSOT content fidelity); multi-sheet handling;
  formula-vs-value on export. _(fill)_
