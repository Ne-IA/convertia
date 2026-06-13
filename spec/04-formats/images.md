# 04 — Formats: Images

> Category spec for images. Template demo for the other category files.
> Formats (SSOT): JPG/JPEG, PNG, WEBP, GIF, BMP, TIFF, HEIC/HEIF*, AVIF, ICO;
> SVG as source (rasterised). *patent-encumbered.

## Source → target matrix
<!-- rows = source, cols = target; cell = ✓ (engine) / — / note -->
| src ＼ tgt | JPG | PNG | WEBP | GIF | BMP | TIFF | HEIC | AVIF | ICO |
|-----------|-----|-----|------|-----|-----|------|------|------|-----|
| JPG | … | | | | | | | | |
| PNG | | | | | | | | | |
| … | | | | | | | | | |

_(matrix to be filled — both directions)_

## Engines (candidates)
- Primary raster: libvips / ImageMagick; HEIC/AVIF: libheif/libavif; SVG raster:
  resvg/librsvg. Licence + platform + patent notes. _(decide & fill)_

## Per-format entries
### JPG/JPEG
_(detection, role, targets both ways, engine, options [quality default], lossy,
edge cases: EXIF/orientation, colour profile)_ — **fill**

### PNG
_(… transparency, interlacing, bit depth …)_ — **fill**

### WEBP
_(… lossy/lossless, quality default, animation …)_ — **fill**

### GIF
_(… animation, palette, → video frames is fan-out=parked …)_ — **fill**

### BMP / TIFF / HEIC/HEIF / AVIF / ICO / SVG(source)
_(one entry each — fill)_

## Category-wide
- Default target highlighting (widely-compatible vs modern); metadata/EXIF
  handling policy; colour-profile policy; animation handling; large-image limits.
  _(fill)_
