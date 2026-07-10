//! `crate::detection` — the §1.2 layered content detection (magic-byte sniffing + the bounded
//! pure-Rust structural peeks). The first code to touch untrusted bytes; it runs in-core in safe Rust
//! with no full decode (§2.12.4 — no third-party C/C++ decoder in the trust kernel), so its no-panic
//! discipline is compile-enforced (G4/G14: the crate-root `unwrap_used`/`expect_used`/`panic` deny +
//! the module `indexing_slicing` deny below).
//!
//! ## P3.26 — the §1.2 layered dispatcher skeleton
//! [Build-Session-Entscheidung: P3.26] This box builds the §1.2 four-step strategy AS A DISPATCHER — the
//! bounded [`MAX_HEADER_WINDOW`] header read ([`read_header`]) and [`detect`], which runs
//! **magic → container → text → structural-peek** in order and returns the canonical §1.2
//! [`DetectionOutcome`](crate::domain::DetectionOutcome). Step 1 (magic) is a live table-driven matcher
//! ([`sniff_magic`]) over the [`MAGIC_SIGNATURES`] registry — genuinely EMPTY in P3 because the
//! walking-skeleton CSV/TSV are magic-less and every per-format signature is §04-owned, added by the format
//! phases P5–P7. The other three steps are the §1.2 order's typed seams filled by their named boxes: text
//! classification by **P3.27** (BOM→UTF-8→codepage encoding) + **P3.28** (CSV-vs-TSV delimiter); container
//! introspection + the structural-peek notes/dims by **P5–P7**; the eligibility / `UnsupportedType` /
//! confidence outcome-rules refinement by **P3.29**. The end-to-end drop→detect→group wiring is **P3.49**.

#![deny(clippy::indexing_slicing)]
// §1.2 in-core untrusted-byte path (T5): indexing/slicing is denied at the module root so an
// out-of-bounds index can never become an in-core panic/DoS. The G4 REQUIRED_ATTRS contract makes
// this deny mandatory the moment this module exists; the crate-root no-panic deny (main.rs) covers the
// rest of the class here.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.26 — the §1.2 dispatcher skeleton (`read_header`/`detect`/`sniff_magic` + the \
                  `MAGIC_SIGNATURES` registry) is the layered-detection FRAMEWORK, authored before its \
                  consumers: the orchestrator drop→detect→group caller is P3.49, the per-step bodies are \
                  P3.27 (text encoding) / P3.28 (delimiter) / P3.29 (outcome rules) / P5–P7 (magic + \
                  container + structural signatures) — so the skeleton is dead in the production build \
                  until P3.49 wires it."
    )
)]

use std::io::{self, Read};

use crate::domain::{Confidence, DetectionOutcome, UserFacingFormat};

/// The §1.2 bounded header window — detection reads at most the first **4 KiB** (§1.2 step 1: "recommended
/// read: first 4 KiB"), a bounded memory-safe read, never a full decode (§2.12.4). The larger bounded
/// structural reads some steps need (a ZIP central-directory peek, a trailer probe) are added WITH their step
/// (P5–P7), each still a bounded member read — never an unbounded slurp of the whole file.
/// [Build-Session-Entscheidung: P3.26]
pub const MAX_HEADER_WINDOW: usize = 4096;

/// Read at most [`MAX_HEADER_WINDOW`] bytes from `reader` — the §1.2 bounded header read every detection step
/// works over. [`Read::take`] caps `read_to_end` so no source, however large or streaming, can make the
/// in-core buffer exceed the 4-KiB window: this bounds **memory** (the buffer size), regardless of source
/// size.
///
/// It does **not** bound **time**: a blocking source — a FIFO/pipe with no writer, a stalled network mount —
/// can hang a `read` indefinitely, and `take` cannot cap a call that never returns. `read_header` takes a bare
/// `impl Read` (it holds no path/fd to stat), so it owns the memory bound alone; ensuring the reader is a
/// regular file (not a blocking FIFO/device) is the **caller's** concern. Today the §1.1 intake walk admits
/// only `is_file()` candidates at freeze (`crate::orchestrator`); closing the residual freeze→open window with
/// a **pre-open** source file-type check at the read site — the pre-open path-`stat` type-check
/// (`std::fs::metadata`, which never opens so never blocks on a FIFO) that `crate::fs_guard` P3.9 applies to
/// the write/publish parent, not that write-side check itself — is the **P3.49** read-path wiring's job
/// (§2.12.4). `read_header` neither performs nor claims that check (it holds no fd to stat).
///
/// Returns the bytes actually read: fewer than the window for a short file, and **empty for a 0-byte source**
/// — which [`detect`] maps to [`DetectionOutcome::Empty`]. Index-free / panic-free (a bounded `read_to_end`
/// into a growable buffer, no manual indexing under the module `indexing_slicing` deny; the only error path is
/// the `reader`'s own I/O error, propagated). [Build-Session-Entscheidung: P3.26]
pub fn read_header(reader: impl Read) -> io::Result<Vec<u8>> {
    let mut header = Vec::new();
    reader
        .take(MAX_HEADER_WINDOW as u64)
        .read_to_end(&mut header)?;
    Ok(header)
}

/// The §1.2 step-1 magic/signature registry — `(signature-prefix, format)` rows matched (longest, i.e. most
/// specific, prefix first) against the bounded header window. **Empty in P3 by design, not as a placeholder:**
/// the walking-skeleton CSV/TSV are magic-LESS (they classify via step 3 text), and every per-format signature
/// (PNG `89 50 4E 47`, `%PDF-`, RIFF / `ftyp` boxes, EBML `1A 45 DF A3`, …) is §04-owned and added by the
/// format phases P5–P7. It is a real, grown data structure that is genuinely empty until then.
/// [Build-Session-Entscheidung: P3.26]
const MAGIC_SIGNATURES: &[(&[u8], UserFacingFormat)] = &[];

/// §1.2 step 1 — match the bounded `header` against a `(signature-prefix, format)` registry, **longest
/// matching prefix first** (a longer matching prefix is the more specific format), returning the recognized
/// format or `None` (a magic-less input, or no signature row matches). Pure slice `starts_with`, no indexing.
/// Parameterized on `signatures` so the step-1 logic is unit-tested against a synthetic registry while
/// [`detect`] passes the real (P3-empty) [`MAGIC_SIGNATURES`].
///
/// **Equal-length tie invariant (the P5–P7 registry contract).** `max_by_key` returns the LAST of several
/// equal-maximum matches — an insertion-order-dependent result. So the registry MUST be pairwise
/// non-overlapping at equal prefix length: two formats that share a *generic* magic of the same length (e.g.
/// the `50 4B 03 04` ZIP-family DOCX/XLSX/PPTX/ODF, or an `ftyp`-brand container) are disambiguated by §1.2
/// **step 2 container introspection**, never by an arbitrary step-1 winner. A P5–P7 box adding two
/// equal-length prefix-overlapping rows would be relying on order — the invariant forbids it.
/// [Build-Session-Entscheidung: P3.26]
fn sniff_magic(
    header: &[u8],
    signatures: &[(&[u8], UserFacingFormat)],
) -> Option<UserFacingFormat> {
    signatures
        .iter()
        .filter(|(signature, _)| header.starts_with(signature))
        .max_by_key(|(signature, _)| signature.len())
        .map(|&(_, format)| format)
}

/// The §1.2 layered content-detection dispatcher — run the four-step strategy **in order** over a bounded
/// header window and return the canonical §1.2 [`DetectionOutcome`]. The step order this box establishes:
///
/// 1. **magic / signature sniff** ([`sniff_magic`]) — live framework; per-format signature rows added P5–P7.
/// 2. **container introspection** (ZIP / OLE2 / `ftyp` / gzip disambiguation) — a typed seam filled by P5–P7
///    (each a bounded member read; §2.12.4).
/// 3. **text classification** (TXT / MD / CSV / TSV / SVG) — the walking-skeleton path filled by **P3.27**
///    (BOM → UTF-8 → codepage encoding confirmation) + **P3.28** (CSV-vs-TSV delimiter sniff), which return
///    `Recognized { Csv | Tsv }`.
/// 4. **bounded structural-peek** — reads the raster `dims` that augment a `Recognized` outcome **at each
///    site where one is constructed** (the step-1/2/3 recognition points, not as a tail step), and the §1.4
///    `notes` that land on `CollectedSet::Single` downstream (not a `Recognized` field); a typed seam filled
///    by P5–P7.
///
/// A 0-byte header is [`DetectionOutcome::Empty`]; an input that no step recognizes is
/// [`DetectionOutcome::Uncertain`] with no best guess — **never** an extension-fallback guess (SSOT
/// *Recognize files by content*). The full §1.2 eligibility / `UnsupportedType` / `Confidence` outcome rules
/// are refined by **P3.29**. This dispatcher is pure bounded-read safe Rust with no third-party C/C++ decoder,
/// so it runs in-core (§2.12.4 absolute satisfied). [Build-Session-Entscheidung: P3.26]
pub fn detect(header: &[u8]) -> DetectionOutcome {
    // §1.2: a 0-byte source has no bytes to classify → Empty (clear-cut; P3.29 refines the other outcome rules).
    if header.is_empty() {
        return DetectionOutcome::Empty;
    }
    // §1.2 step 1 — magic / signature sniff (the MAGIC_SIGNATURES registry is grown per-format in P5–P7).
    if let Some(format) = sniff_magic(header, MAGIC_SIGNATURES) {
        // §1.2 step 4 — a recognized RASTER format's bounded structural-peek reads the intrinsic `dims`
        //   (JPEG SOF / PNG IHDR / …) that augment THIS Recognized outcome at its construction site (P5–P7
        //   fill it via a bounded member read, so `dims` stays None in P3); mirror this at the step-2/step-3
        //   Recognized sites below. (The §1.4 `notes` are the SAME step-4 peek's other output, but they land
        //   on `CollectedSet::Single` downstream, not on a `Recognized` field.) A magic hit is high-confidence.
        return DetectionOutcome::Recognized {
            format,
            confidence: Confidence::High,
            dims: None,
        };
    }
    // §1.2 step 2 — container introspection (ZIP / OLE2 / ftyp / gzip) inserts its bounded member-read peek
    //   here (P5–P7); a match builds (and step-4-augments) a Recognized outcome as above.
    // §1.2 step 3 — text classification fills the CSV/TSV walking-skeleton path here: P3.27 (encoding) +
    //   P3.28 (delimiter) build (and step-4-augment) Recognized { Csv | Tsv } as above. A magic-less input
    //   falls through to it.
    // §1.2 / SSOT: an input no step recognizes is Uncertain, NEVER an extension-fallback guess. P3.29 refines
    //   this (a recognized-but-unconvertible type → UnsupportedType, the eligibility split, the Confidence rule).
    DetectionOutcome::Uncertain { best_guess: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    // §6.4.1 unit (G15): `read_header` is bounded to the §1.2 4-KiB window — a source larger than the window
    // yields EXACTLY `MAX_HEADER_WINDOW` bytes (a FIFO / huge file can never pull more into the core, §2.12.4).
    #[test]
    fn read_header_caps_at_the_window() {
        let oversize = vec![b'x'; MAX_HEADER_WINDOW * 3];
        let header = read_header(oversize.as_slice()).expect("reading a byte slice never fails");
        assert_eq!(
            header.len(),
            MAX_HEADER_WINDOW,
            "§1.2/§2.12.4: the header read is bounded to the 4-KiB window regardless of source size"
        );
    }

    // §6.4.1 unit (G15): a source shorter than the window is returned VERBATIM in full (the bound is a ceiling,
    // not a fixed-size read), and a 0-byte source yields an empty header.
    #[test]
    fn read_header_returns_a_short_source_verbatim_and_empty_for_zero_bytes() {
        let short = b"id,name\n1,a\n";
        let header = read_header(short.as_slice()).expect("reading a byte slice never fails");
        assert_eq!(
            header.as_slice(),
            short,
            "§1.2: a source shorter than the window is read in full, byte-for-byte"
        );
        let empty_src: &[u8] = &[];
        let empty = read_header(empty_src).expect("reading an empty slice never fails");
        assert!(
            empty.is_empty(),
            "§1.2: a 0-byte source yields an empty header (detect maps that to Empty)"
        );
    }

    // §6.4.1 unit (G15): `detect` maps a 0-byte header to §1.2 `Empty` (the clear-cut step-0 rule).
    #[test]
    fn detect_maps_an_empty_header_to_empty() {
        assert_eq!(
            detect(&[]),
            DetectionOutcome::Empty,
            "§1.2: a 0-byte source is Empty, never Uncertain/Recognized"
        );
    }

    // §6.4.1 unit (G15): the §1.2 "recognize by content, never by extension" property — an input that no step
    // recognizes (non-text, no magic) is `Uncertain { best_guess: None }`, NOT an extension-fallback guess.
    // This is stable across the P3.27/P3.28 text-step fill (a non-text input never becomes Recognized) and is
    // the honest P3.26-skeleton fall-through (the magic registry is empty; container/text/structural are seams).
    #[test]
    fn detect_falls_through_to_uncertain_never_guessing() {
        let non_text_no_magic = [0x00_u8, 0x01, 0x02, 0x03, 0xFF, 0xFE];
        assert_eq!(
            detect(&non_text_no_magic),
            DetectionOutcome::Uncertain { best_guess: None },
            "§1.2/SSOT: an unrecognized input is Uncertain — never extension-fallback-guessed"
        );
    }

    // §6.4.1 unit (G15): the P3 magic registry is genuinely EMPTY (CSV/TSV are magic-less; per-format
    // signatures are P5–P7), so step-1 `sniff_magic` recognizes nothing yet — pinning the honest P3 state so a
    // future accidental population is a visible, reviewed change, not a silent one.
    #[test]
    fn magic_registry_is_empty_in_p3() {
        assert!(
            MAGIC_SIGNATURES.is_empty(),
            "§1.2: the magic registry is empty until P5–P7 add per-format signatures (CSV/TSV are magic-less)"
        );
        assert_eq!(
            sniff_magic(b"anything at all", MAGIC_SIGNATURES),
            None,
            "§1.2: with an empty registry, step-1 magic sniff recognizes no format"
        );
    }

    // §6.4.1 unit (G15): step-1 `sniff_magic` classification over a SYNTHETIC registry (the real one is
    // P3-empty) — exercises the Recognized path P5–P7 will populate: a signature prefix matches its format,
    // the LONGEST matching prefix wins (a more specific signature beats a shorter one that also prefix-matches),
    // and a non-matching header recognizes nothing. This pins the longest-prefix rule so a P5–P7 signature
    // addition inherits a tested matcher, not an untested one.
    #[test]
    fn sniff_magic_matches_longest_prefix() {
        // A generic 2-byte prefix shared with a more-specific 4-byte one (the shape §1.2 warns about — here
        // resolved by length, the disambiguation the equal-length invariant reserves for step 2).
        let registry: &[(&[u8], UserFacingFormat)] = &[
            (b"BM", UserFacingFormat::Bmp),
            (b"\x89PNG", UserFacingFormat::Png),
            (b"\x89P", UserFacingFormat::Gif), // deliberately shorter + wrong: the longer \x89PNG must win
        ];
        assert_eq!(
            sniff_magic(b"BMxxxx", registry),
            Some(UserFacingFormat::Bmp),
            "§1.2: a signature prefix classifies its format"
        );
        assert_eq!(
            sniff_magic(b"\x89PNG\r\n", registry),
            Some(UserFacingFormat::Png),
            "§1.2: the LONGEST matching prefix wins — \\x89PNG (4) beats the shorter \\x89P (2)"
        );
        assert_eq!(
            sniff_magic(b"not a match", registry),
            None,
            "§1.2: a header matching no signature recognizes nothing (never extension-guessed)"
        );
    }
}
