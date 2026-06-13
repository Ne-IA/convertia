# 04 — Formats: Audio

> Formats (SSOT): MP3, WAV, FLAC, AAC*, M4A, OGG, OPUS, WMA, AIFF, ALAC.
> *patent-encumbered. Follows the per-format template in [README](README.md).

## Source → target matrix
_(rows = source, cols = target; both directions — fill)_

## Engine(s)
- Primary: FFmpeg (LGPL build) — codecs, AAC patent note per platform. _(fill)_

## Per-format entries
_(MP3, WAV, FLAC, AAC, M4A, OGG, OPUS, WMA, AIFF, ALAC — one each: detection,
targets both ways, options [bitrate/quality defaults], lossy, edge cases:
metadata/tags, sample rate, channels, container vs codec) — **fill**_

## Category-wide
- Tag/metadata preservation; bitrate/quality defaults; lossless↔lossy disclosure.
  _(fill)_
