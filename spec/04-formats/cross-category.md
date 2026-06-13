# 04 — Formats: Cross-category outputs (closed set)

> The only cross-category conversions in v1 (SSOT *Direction & shape rule*):
> **extract-audio** (video → audio) and **to-animated-GIF** (video → GIF).
> A closed set; anything else is parked. Follows the template in [README](README.md).

## Extract audio (video → MP3/WAV/M4A/…)
- Sources: all video formats. Targets: the audio set (which subset? — resolve).
- Engine: FFmpeg (stream copy where possible vs re-encode). Options + defaults.
- Lossy notes; edge cases (no audio track, multiple tracks). _(fill)_

## To animated GIF (video → GIF)
- Sources: all video formats. Engine: FFmpeg (palette gen). Options: fps, size,
  duration/trim? (decide v1 scope) + defaults. Lossy (palette). Edge cases (very
  long videos → huge GIF — guardrail). _(fill)_

## Interaction notes
- These appear as additional **targets** of a video source (not a second source
  format); batch rule still keys on the video source type. _(fill)_
