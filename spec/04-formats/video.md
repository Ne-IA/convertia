# 04 — Formats: Video

> Formats (SSOT): MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP.
> Cross-category outputs (extract-audio, to-GIF) live in
> [cross-category.md](cross-category.md). Follows the template in [README](README.md).

## Source → target matrix
_(rows = source, cols = target; both directions — fill)_

## Engine(s)
- Primary: FFmpeg — container/codec handling, hardware vs software, patent notes.
  _(fill)_

## Per-format entries
_(MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP — one each: detection,
targets both ways, options [resolution/codec/quality defaults], lossy, edge
cases: codec inside container, audio tracks, subtitles, very large files,
long-running progress) — **fill**_

## Category-wide
- Re-encode vs remux policy; default codec/quality; progress for long jobs;
  size/time expectations. _(fill)_
