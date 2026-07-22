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

use encoding_rs::{Encoding, UTF_8};

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
/// 3. **text classification** (TXT / MD / CSV / TSV / SVG) — the walking-skeleton path: **P3.27** (BOM →
///    UTF-8 → codepage encoding confirmation) + **P3.28** (CSV-vs-TSV delimiter sniff), **wired here by P3.29**
///    to return `Recognized { Csv | Tsv }` for a consistent delimiter (TXT / MD / SVG are a subsequent-phase fill).
/// 4. **bounded structural-peek** — reads the raster `dims` that augment a `Recognized` outcome **at each
///    site where one is constructed** (the step-1/2/3 recognition points, not as a tail step), and the §1.4
///    `notes` that land on `CollectedSet::Single` downstream (not a `Recognized` field); a typed seam filled
///    by P5–P7.
///
/// A 0-byte header is [`DetectionOutcome::Empty`]; an input that no step recognizes is
/// [`DetectionOutcome::Uncertain`] with no best guess — **never** an extension-fallback guess (SSOT
/// *Recognize files by content*). The §1.2 eligibility / `Confidence` outcome rules are applied here by
/// **P3.29** (a consistent CSV/TSV delimiter ⇒ `Recognized … High`; text-but-not-delimited or non-text ⇒
/// `Uncertain`; `UnsupportedType` needs a magic/container match to name the type, so no P3 input reaches it).
/// This dispatcher is pure bounded-read safe Rust with no third-party C/C++ decoder, so it runs in-core
/// (§2.12.4 absolute satisfied). [Build-Session-Entscheidung: P3.26]
pub fn detect(header: &[u8]) -> DetectionOutcome {
    // §1.2: a 0-byte source has no bytes to classify → Empty (clear-cut; the other outcome rules follow below).
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
    // §1.2 step 3 — text classification (P3.29 wires the walking-skeleton CSV/TSV path). Confirm the bytes
    //   decode as text (P3.27 `classify_encoding`: BOM → UTF-8 → codepage); if so, sniff the delimiter (P3.28
    //   `classify_delimiter`) and, on a consistent CSV/TSV delimiter, build the `Recognized { Csv | Tsv }`
    //   outcome. `detect` is EXTENSION-FREE (it classifies bytes, not names — §1.2 "never trusting the
    //   extension"), so the delimiter tie-break gets NO extension hint here (`None`); the extension is only a
    //   last-resort tie-breaker the end-to-end wiring (P3.49) threads if it gives `detect` a path, so the
    //   content decides on its own here. [Build-Session-Entscheidung: P3.29]
    if let Some(encoding) = classify_encoding(header) {
        if let DelimiterClass::Detected(delimiter) = classify_delimiter(header, encoding, None) {
            // §1.2 step 4: CSV/TSV are non-raster, so the structural-peek `dims` are None. Confidence is High —
            //   P3.28 returns `Detected` only on a strict-majority CONSISTENT delimiter across ≥ 2 records, an
            //   unambiguous content signal (the peer of a magic hit's High); `Confidence::Low` is reserved for a
            //   genuinely-weak signal a subsequent detection path may surface, which a strict-majority delimiter is not.
            //   [Build-Session-Entscheidung: P3.29]
            return DetectionOutcome::Recognized {
                format: delimiter.user_facing_format(),
                confidence: Confidence::High,
                dims: None,
            };
        }
        // Confirmed text but NOT a consistent CSV/TSV — an ambiguous delimiter, or a non-delimited text format
        //   (TXT / MD / SVG) whose classification is a subsequent-phase fill — falls through to Uncertain in the P3
        //   walking skeleton, never a wrong CSV/TSV guess.
    }
    // §1.2 / SSOT outcome rule: an input no step recognizes — non-text/binary with no magic, or confirmed text
    //   with no consistent CSV/TSV delimiter — is `Uncertain { best_guess: None }`, surfaced eligible=false and
    //   NEVER extension-fallback-guessed. An `UnsupportedType` (a real type we identify but do not convert)
    //   needs a magic/container match to NAME the type, so the empty P3 registry never reaches it; the §1.3
    //   projection maps this `Uncertain` to `SkipReason::Uncertain`. [Build-Session-Entscheidung: P3.29]
    DetectionOutcome::Uncertain { best_guess: None }
}

/// §1.2 step-3 text-encoding classification — decide whether the bounded `header` decodes as text and, if so,
/// WHICH encoding, **detected from content, never assumed from the extension** (§2.10.2). The order is §1.2
/// step 3's "BOM → strict UTF-8 → single-byte codepage fallback" (§2.10.2's "declared charset" step — `<meta>`
/// / XML-decl / RTF code page — is format-specific and §04/engine-owned, so it is not a step here; the
/// magic-less TEXT formats TXT/MD/CSV/TSV carry no declared charset):
///
/// 0. **UTF-32 declines** — encoding_rs has no UTF-32 support and its `for_bom` would alias a UTF-32LE BOM
///    (`FF FE 00 00`) to UTF-16LE, a confidently-WRONG result §2.10.2 forbids ("mixed/invalid → fail clearly");
///    ConvertIA does not support UTF-32 (WHATWG omits it), so a UTF-32 BOM returns `None` (the dispatcher's
///    `Uncertain`), never mis-mapped.
/// 1. **BOM** is authoritative for the supported encodings (`Encoding::for_bom` — UTF-8 / UTF-16 LE|BE only).
/// 2. **Binary guard:** a NUL byte means these are not one of the magic-less TEXT formats (TXT / MD / CSV / TSV
///    / SVG are NUL-free); BOM-less UTF-16 (which is NUL-bearing) is the caught-by-BOM common case, so a
///    residual NUL ⇒ not text ⇒ `None` (the dispatcher's `Uncertain`).
/// 3. **strict UTF-8** — the modern default (§2.10.2 output default): valid UTF-8, *allowing a multi-byte char
///    truncated at the [`MAX_HEADER_WINDOW`] boundary* (that is an incomplete final char, not invalid bytes).
/// 4. **single-byte codepage fallback** via `chardetng` (§2.10.2 "heuristic UTF-8 → Windows-1252/Latin-1 →
///    broader") — a pure-Rust bounded heuristic over the window; it always yields a best-guess encoding for
///    NUL-free non-UTF-8 bytes.
///
/// In-core, bounded (works over the already-bounded header window), memory-safe Rust — `chardetng` +
/// `encoding_rs` are pure Rust with no third-party C/C++ decoder (§2.12.4). Index-free / panic-free.
/// [Build-Session-Entscheidung: P3.27]
pub fn classify_encoding(header: &[u8]) -> Option<&'static Encoding> {
    // (0) UTF-32 is unsupported: encoding_rs has no UTF-32, and its `for_bom` would alias a UTF-32LE BOM
    //     `FF FE 00 00` to UTF-16LE (a confidently-wrong result, §2.10.2). Decline both UTF-32 BOMs to None
    //     BEFORE for_bom so UTF-32 is an honest "can't handle", never mis-mapped. (The BE BOM `00 00 FE FF`
    //     would also be caught by the NUL guard below; declining it here keeps the UTF-32 handling explicit +
    //     symmetric.)
    const UTF32_LE_BOM: &[u8] = &[0xFF, 0xFE, 0x00, 0x00];
    const UTF32_BE_BOM: &[u8] = &[0x00, 0x00, 0xFE, 0xFF];
    if header.starts_with(UTF32_LE_BOM) || header.starts_with(UTF32_BE_BOM) {
        return None;
    }
    // (1) An explicit UTF-8 / UTF-16 BOM is authoritative (§2.10.2).
    if let Some((encoding, _bom_len)) = Encoding::for_bom(header) {
        return Some(encoding);
    }
    // (2) Binary guard: a NUL byte ⇒ not a magic-less TEXT format ⇒ not text.
    if header.contains(&0) {
        return None;
    }
    // (3) strict UTF-8, tolerating a window-boundary-truncated trailing multi-byte char.
    if is_valid_utf8_allowing_truncation(header) {
        return Some(UTF_8);
    }
    // (4) chardetng single-byte codepage fallback (a bounded pure-Rust heuristic; always yields a guess).
    //     Iso2022JpDetection::Deny — the security-conservative choice for a non-script file detector (chardetng
    //     doc: only email-class decoders should Allow the stateful escape-based encoding); Utf8Detection::Deny —
    //     step 3 already ruled out valid UTF-8, so the codepage fallback must never re-guess UTF-8.
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(header, /* last = */ true);
    Some(detector.guess(/* tld = */ None, chardetng::Utf8Detection::Deny))
}

/// Valid UTF-8, **allowing a trailing multi-byte char truncated at the [`MAX_HEADER_WINDOW`] boundary** — the
/// 4-KiB read can cut a source mid-character, and `Utf8Error::error_len()` is `None` exactly for that
/// "unexpected end of input" case (vs `Some(n)` for a genuinely invalid byte sequence).
///
/// The trailing-incomplete-char tolerance is gated on the header FILLING the window (`len >=
/// MAX_HEADER_WINDOW`): only then is the incomplete char a genuine window cut of a longer UTF-8 stream. When
/// the whole (shorter) source fit in the window, a trailing incomplete char is the ACTUAL file end — a real
/// UTF-8 file never ends mid-character — so it is genuinely-invalid UTF-8 and declines here, falling to the
/// codepage heuristic (so e.g. a short Windows-1252 `"café"` ending in a lone `0xE9` is NOT mis-read as a
/// truncated-UTF-8 false positive). Real mid-stream mojibake (`error_len() == Some`) always declines (§2.10.2
/// "mixed/invalid → fail clearly"). Panic-free / index-free. [Build-Session-Entscheidung: P3.27]
fn is_valid_utf8_allowing_truncation(header: &[u8]) -> bool {
    match std::str::from_utf8(header) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && header.len() >= MAX_HEADER_WINDOW,
    }
}

/// The §1.4 `CollectedSummary.encoding_hint` projection — the encoding name for a NON-default detected
/// encoding, or `None` for **UTF-8** (the §2.10.2 assumed default needs no hint) and for a non-text input.
/// Fed from [`classify_encoding`].
///
/// **The emitted string is `encoding_rs`'s canonical WHATWG label** (`"windows-1252"`, `"Shift_JIS"`,
/// `"ISO-8859-2"`, …) — the honest, standard encoding identity. The §2.10.2 / plan examples spell it
/// `"Windows-1252"` (capitalised), but those are **illustrative** (the plan writes *"e.g. Windows-1252"*), not
/// a normative string contract — the WHATWG canonical is lowercase only for the `windows-*` family. Any
/// prettier user-facing display casing is a §5 UI presentation concern (surfaced as a Co-Pilot note), not the
/// detection layer's — this layer produces the canonical machine identity. [Build-Session-Entscheidung: P3.27]
pub fn encoding_hint(encoding: &'static Encoding) -> Option<String> {
    if encoding == UTF_8 {
        None
    } else {
        Some(encoding.name().to_owned())
    }
}

// ─── P3.28 — §1.2 step-3 CSV-vs-TSV delimiter sniff (content over name, spreadsheets.md) ──────────────

/// The number of leading NON-EMPTY records the delimiter sniff samples — spreadsheets.md "Delimiter
/// detection policy" ("the most consistent field-count across the sample (default first **20** non-empty
/// lines)"). A bounded sample over the already-bounded [`MAX_HEADER_WINDOW`] header window, never the whole
/// file. [Build-Session-Entscheidung: P3.28]
const DELIMITER_SNIFF_SAMPLE_LINES: usize = 20;

/// A delimiter candidate the §1.2 CSV/TSV sniff probes — spreadsheets.md "Candidates probed: comma `,`,
/// semicolon `;`, tab `\t`, pipe `|`". All four are ASCII, so the sniff is **encoding-independent** once
/// the header is decoded to text (a comma is one `,` whether the source was UTF-8, UTF-16, or a
/// Windows-1252 codepage — [`classify_delimiter`] decodes first). The [`CANDIDATES`](Delimiter::CANDIDATES)
/// declaration order IS the final deterministic tie-break order — comma first (spreadsheets.md "then
/// comma"). [Build-Session-Entscheidung: P3.28]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    /// The four candidates in tie-break priority order — comma first (spreadsheets.md "then comma"), then a
    /// fixed order that keeps a beyond-comma tie deterministic. Each entry's position is the [`RecordCounts`]
    /// slot [`candidate_index`] maps its char to (a G15 test locks the alignment).
    const CANDIDATES: [Delimiter; 4] = [
        Delimiter::Comma,
        Delimiter::Semicolon,
        Delimiter::Tab,
        Delimiter::Pipe,
    ];

    /// The literal character this candidate splits on.
    const fn as_char(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }

    /// The §1.3 user-facing format a file with this dominant delimiter groups under — **tab ⇒ TSV**, every
    /// other delimiter ⇒ **CSV** (spreadsheets.md "A tab winner reclassifies the file as TSV"; a semicolon-
    /// or pipe-separated file stays CSV). Content over name: this is the SNIFFED delimiter's format, never
    /// the extension's (§1.3 grouping key, CSV ≠ TSV delimiter-determined). [Build-Session-Entscheidung: P3.28]
    const fn user_facing_format(self) -> UserFacingFormat {
        match self {
            Delimiter::Tab => UserFacingFormat::Tsv,
            Delimiter::Comma | Delimiter::Semicolon | Delimiter::Pipe => UserFacingFormat::Csv,
        }
    }
}

/// The extension's delimiter SUGGESTION — used ONLY as the §1.2 tie-break between two content
/// interpretations that are EQUALLY consistent (spreadsheets.md "a tie-break preferring the extension hint
/// then comma"), NEVER as a primary or fallback signal. Content always decides the winner when there is a
/// clear one: a `.csv` whose bytes are consistently tab-separated is **TSV** (content over name), and a
/// `.csv` with NO consistent delimiter is [`Ambiguous`](DelimiterClass::Ambiguous) → `Uncertain` (never
/// rescued to comma by the extension — the "never silently extension-fall-back" rule, §1.2). The extension
/// only disambiguates a genuine tie between ≥ 2 equally-consistent delimiters. [Build-Session-Entscheidung: P3.28]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionDelimiterHint {
    /// A `.csv` extension → prefer comma on a genuine tie.
    Comma,
    /// A `.tsv` / `.tab` extension → prefer tab on a genuine tie.
    Tab,
}

impl ExtensionDelimiterHint {
    /// Derive the tie-break hint from a file's (dot-less, any-case) extension — `csv` ⇒ [`Comma`](Self::Comma),
    /// `tsv` / `tab` ⇒ [`Tab`](Self::Tab) (the extensions spreadsheets.md names for CSV/TSV), `None` for any
    /// other extension (which lends no tie-break signal). The raw path → extension extraction is the P3.49
    /// caller's concern; this maps an already-extracted extension string. [Build-Session-Entscheidung: P3.28]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "csv" => Some(ExtensionDelimiterHint::Comma),
            "tsv" | "tab" => Some(ExtensionDelimiterHint::Tab),
            _ => None,
        }
    }

    /// The delimiter this extension hint prefers on a tie.
    const fn preferred(self) -> Delimiter {
        match self {
            ExtensionDelimiterHint::Comma => Delimiter::Comma,
            ExtensionDelimiterHint::Tab => Delimiter::Tab,
        }
    }
}

/// The result of the §1.2 CSV-vs-TSV delimiter sniff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterClass {
    /// A dominant delimiter was found consistently across the sample — the file is delimited text; the
    /// [`Delimiter`] decides CSV vs TSV ([`user_facing_format`](DelimiterClass::user_facing_format)) and the
    /// [`delimiter_hint`].
    Detected(Delimiter),
    /// No candidate delimiter produced a consistent multi-field split across the sample — the sniff declines
    /// (the §1.2 dispatcher maps this to `Uncertain`), **never** an extension fallback (§1.2 / SSOT
    /// *Recognize files by content*: a `.csv` with no consistent delimiter is declined, not assumed comma).
    Ambiguous,
}

impl DelimiterClass {
    /// The §1.3 grouping key this classification yields — `Some(Csv | Tsv)` for a detected delimiter, `None`
    /// when [`Ambiguous`](DelimiterClass::Ambiguous) (→ the dispatcher's `Uncertain`). CSV ≠ TSV is
    /// delimiter-determined here, the §1.3 content-over-name key. [Build-Session-Entscheidung: P3.28]
    pub fn user_facing_format(self) -> Option<UserFacingFormat> {
        match self {
            DelimiterClass::Detected(delimiter) => Some(delimiter.user_facing_format()),
            DelimiterClass::Ambiguous => None,
        }
    }
}

/// The §1.4 `CollectedSummary.delimiter_hint` projection — the display delimiter for a **non-obvious**
/// separator, or `None` when the delimiter is the format's canonical default (**comma** for CSV, **tab** for
/// TSV — mirroring [`encoding_hint`]'s `None`-for-UTF-8) and for an ambiguous sniff. It surfaces the
/// **semicolon** (European-Excel `;`-CSV, where a comma is a decimal point) and **pipe** cases so the §1.4
/// confirm summary can show the detected separator (spreadsheets.md "The detected delimiter is surfaced in
/// the collected summary") — a CSV whose delimiter is the expected comma, and a TSV (tab is definitional to
/// the format label), need no disclosure. Any prettier display casing is a §5 UI concern, like the
/// [`encoding_hint`] casing note. [Build-Session-Entscheidung: P3.28]
pub fn delimiter_hint(class: DelimiterClass) -> Option<String> {
    match class {
        DelimiterClass::Detected(Delimiter::Semicolon) => Some(";".to_owned()),
        DelimiterClass::Detected(Delimiter::Pipe) => Some("|".to_owned()),
        DelimiterClass::Detected(Delimiter::Comma | Delimiter::Tab) | DelimiterClass::Ambiguous => {
            None
        }
    }
}

/// §1.2 step 3 (CSV/TSV) — sniff the dominant delimiter over the decoded, bounded header and classify the
/// file as CSV or TSV **by content, never by extension** (spreadsheets.md "Delimiter detection policy";
/// §2.10.2 "CSV encoding + delimiter … are detected and preserved … never silently re-delimited").
///
/// `encoding` is the P3.27-detected text encoding — this step runs ONLY on confirmed text (i.e.
/// [`classify_encoding`] returned `Some`) — and the header is decoded through it so the ASCII delimiters are
/// counted correctly even for UTF-16, where a comma is the two bytes `2C 00`. `ext_hint` is the OPTIONAL
/// §1.2 tie-break; the extension-free [`detect`] path passes `None` and gets the "then comma" secondary
/// tie-break.
///
/// Algorithm (spreadsheets.md): over the first [`DELIMITER_SNIFF_SAMPLE_LINES`] non-empty records a
/// candidate is *viable* iff a **strict majority of at least two records** share the SAME occurrence count
/// `≥ 1` (a consistent, repeated ≥ 2-field split — one incidental delimiter in a single prose line does not
/// count); the winner is the viable candidate the most records agree on, ties broken by `ext_hint` → comma →
/// the [`Delimiter::CANDIDATES`] order. No viable candidate (single-column text, prose,
/// empty) ⇒ [`DelimiterClass::Ambiguous`], a clear decline. Counting is RFC-4180 quote-aware (a delimiter
/// inside a `"…"` field is literal, so a `;`-CSV with a `1,50` decimal is not mis-split on the comma,
/// §2.10.2). Bounded, in-core, memory-safe Rust — no third-party C/C++ decoder (§2.12.4); index-free /
/// panic-free under the module `indexing_slicing` deny. [Build-Session-Entscheidung: P3.28]
pub fn classify_delimiter(
    header: &[u8],
    encoding: &'static Encoding,
    ext_hint: Option<ExtensionDelimiterHint>,
) -> DelimiterClass {
    let (text, _, _) = encoding.decode(header);
    sniff_delimiter(&text, ext_hint)
}

/// The pure-text core of [`classify_delimiter`] (unit-tested directly with `&str`): sample the leading
/// non-empty records and pick the most-consistent viable delimiter with the §1.2 tie-break. Split out so the
/// sniff logic is tested without a decode round-trip. [Build-Session-Entscheidung: P3.28]
fn sniff_delimiter(text: &str, ext_hint: Option<ExtensionDelimiterHint>) -> DelimiterClass {
    let sample = sample_record_counts(text);
    // (candidate, agreement) for every VIABLE candidate, in CANDIDATES (comma-first) order.
    let viable: Vec<(Delimiter, usize)> = Delimiter::CANDIDATES
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            viable_agreement(&sample, index).map(|agreement| (candidate, agreement))
        })
        .collect();
    let Some(best_agreement) = viable.iter().map(|&(_, agreement)| agreement).max() else {
        return DelimiterClass::Ambiguous;
    };
    // Tie-break among the best-agreement candidates: the ext_hint's preferred delimiter if it is one of them,
    // else the first in CANDIDATES order (comma-first → "then comma"). `viable` is already in CANDIDATES
    // order, so the fallback `find` yields comma-first on a tie.
    let preferred = ext_hint.map(ExtensionDelimiterHint::preferred);
    let winner = viable
        .iter()
        .filter(|&&(_, agreement)| agreement == best_agreement)
        .find(|&&(candidate, _)| Some(candidate) == preferred)
        .or_else(|| {
            viable
                .iter()
                .find(|&&(_, agreement)| agreement == best_agreement)
        })
        .map(|&(candidate, _)| candidate);
    // `winner` is always `Some` here (best_agreement came from a non-empty `viable`); the total match keeps
    // the selection panic-free even if that invariant ever changes.
    match winner {
        Some(candidate) => DelimiterClass::Detected(candidate),
        None => DelimiterClass::Ambiguous,
    }
}

/// Per-record occurrence counts of the four [`Delimiter::CANDIDATES`] OUTSIDE quoted regions — index `i` is
/// `CANDIDATES[i]`.
type RecordCounts = [usize; 4];

/// Scan `text` into per-record delimiter counts in ONE RFC-4180 quote-aware pass: a `\n` outside quotes ends
/// a record; a candidate delimiter outside quotes increments its slot; everything inside a `"…"` field (with
/// `""` an escaped quote) is literal and counts nothing. Returns the counts for the first
/// [`DELIMITER_SNIFF_SAMPLE_LINES`] records that carry any non-whitespace content (blank lines skipped —
/// spreadsheets.md "non-empty lines"). Counting during the split (rather than re-scanning record strings)
/// keeps the quote structure authoritative and allocates no per-record strings.
///
/// **Field-position-aware quoting (RFC-4180).** A `"` opens a quoted field ONLY when it is the FIRST
/// character of a field (`at_field_start` — set at input start and after each unquoted delimiter/newline); a
/// `"` anywhere else is a literal field character, NOT a quote toggle. Without this a bare mid-field quote
/// (an inch mark `5' 10"`, an informal quote) would open a spurious quoted region that swallows a real
/// `\n`/delimiter and merges records, skewing the sniff toward a wrong or `Ambiguous` result. Index-free
/// (`.get_mut` only) / panic-free (`saturating_add`). [Build-Session-Entscheidung: P3.28]
fn sample_record_counts(text: &str) -> Vec<RecordCounts> {
    let mut records: Vec<RecordCounts> = Vec::new();
    let mut counts: RecordCounts = [0; 4];
    let mut has_content = false;
    let mut in_quotes = false;
    // RFC-4180: a `"` opens a quoted field only at the start of a field — true at input start and after each
    // unquoted delimiter/record boundary, false once any field content has been seen.
    let mut at_field_start = true;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next(); // RFC-4180 escaped "" — consume, stay quoted
                } else {
                    in_quotes = false; // closing quote; the next char is expected to be a delimiter/newline
                }
            }
            has_content = true;
            continue;
        }
        match ch {
            // a field-initial `"` opens a quoted field; a `"` mid-field falls through to `_` as a literal.
            '"' if at_field_start => {
                in_quotes = true;
                has_content = true;
                at_field_start = false;
            }
            '\n' => {
                if has_content {
                    records.push(counts);
                    if records.len() >= DELIMITER_SNIFF_SAMPLE_LINES {
                        return records;
                    }
                }
                counts = [0; 4];
                has_content = false;
                at_field_start = true;
            }
            '\r' => {
                // A CR is the lead byte of a CRLF when a `\n` follows — then it is ignored (not content, does
                // not move the field position) and the `\n` arm closes the record. A LONE CR (no following
                // `\n`) is a classic-Mac / legacy-exporter record terminator (§04 spreadsheets "mixed line
                // endings (CRLF/LF/CR) normalised on read") and must close the record itself — without this a
                // CR-delimited file collapses to ONE sampled record and is wrongly declined `Uncertain` (P3.75
                // sweep). A CR inside a quoted field is handled by the `in_quotes` arm above, never here.
                if chars.peek() != Some(&'\n') {
                    if has_content {
                        records.push(counts);
                        if records.len() >= DELIMITER_SNIFF_SAMPLE_LINES {
                            return records;
                        }
                    }
                    counts = [0; 4];
                    has_content = false;
                    at_field_start = true;
                }
            }
            _ => {
                if let Some(index) = candidate_index(ch) {
                    if let Some(slot) = counts.get_mut(index) {
                        *slot = slot.saturating_add(1);
                    }
                    has_content = true;
                    at_field_start = true; // a new field begins after an unquoted delimiter
                } else {
                    if !ch.is_whitespace() {
                        has_content = true;
                    }
                    // a plain space lends no content (a spaces-only line stays "empty"), but ANY non-delimiter
                    // char — space, letter, or a non-field-initial `"` — is field body, so the next `"` is literal.
                    at_field_start = false;
                }
            }
        }
    }
    if has_content {
        records.push(counts);
    }
    records.truncate(DELIMITER_SNIFF_SAMPLE_LINES);
    records
}

/// The [`Delimiter::CANDIDATES`] slot a delimiter char occupies, or `None` — the counting inverse of
/// [`Delimiter::as_char`]. Kept in lock-step with `CANDIDATES` by a G15 test. [Build-Session-Entscheidung: P3.28]
const fn candidate_index(ch: char) -> Option<usize> {
    match ch {
        ',' => Some(0),
        ';' => Some(1),
        '\t' => Some(2),
        '|' => Some(3),
        _ => None,
    }
}

/// Is the candidate at [`RecordCounts`] slot `index` a *viable* delimiter over `sample`, and if so how many
/// records agree with its modal split? Viable ⇔ some occurrence count `k ≥ 1` is shared by a **strict
/// majority of at least TWO records** (`agreement ≥ 2 ∧ 2 · agreement > len`) — a consistent ≥ 2-field split
/// that is genuinely repeated (spreadsheets.md "the most consistent field-count across the sample").
///
/// **Consistency is a cross-line property.** The `agreement ≥ 2` floor is load-bearing: a single line with
/// one incidental delimiter (prose — `"Hello, world"`, a log line with a comma) is NOT a consistent split
/// and must **decline** (§1.2 / spreadsheets.md "ambiguous → decline clearly … never a wrong split"), never
/// classify as CSV off one unverifiable observation. So the sniff needs at least two records that AGREE on
/// the same `k ≥ 1` split, and those must be a strict majority — a delimiter present in only some rows
/// (`"Hello, world\nGoodbye"`, comma count `[1, 0]`) is inconsistent and declines. Returns that `agreement`
/// (the ranking score) or `None` for the "no consistent delimiter" decline. Among counts with equal
/// agreement the smaller `k` is tracked for determinism; only the agreement is returned. Index-free /
/// panic-free. [Build-Session-Entscheidung: P3.28]
fn viable_agreement(sample: &[RecordCounts], index: usize) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None; // (count k ≥ 1, agreement — records with exactly k)
    for record in sample {
        let Some(&k) = record.get(index) else {
            continue;
        };
        if k == 0 {
            continue;
        }
        let agreement = sample
            .iter()
            .filter(|other| other.get(index) == Some(&k))
            .count();
        best = Some(match best {
            None => (k, agreement),
            Some((best_k, best_agreement))
                if agreement > best_agreement || (agreement == best_agreement && k < best_k) =>
            {
                (k, agreement)
            }
            Some(current) => current,
        });
    }
    let (_, agreement) = best?;
    // A strict majority of ≥ 2 records: at least two records agree (`agreement >= 2`, so a lone incidental
    // delimiter cannot classify) AND they are more than half the sample (`2·agreement > len`, so a delimiter
    // missing from some rows is inconsistent and declines).
    (agreement >= 2 && agreement.saturating_mul(2) > sample.len()).then_some(agreement)
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

    // §6.4.1 unit (G15): §1.2/§2.10.2 BOM handling — a supported BOM is authoritative and wins over the binary
    // guard (UTF-8; UTF-16 LE|BE, whose NUL bytes would otherwise trip the guard, because BOM is checked
    // first). A UTF-32 BOM is UNSUPPORTED and DECLINES to None — never mis-mapped to UTF-16LE (encoding_rs's
    // `for_bom` would alias the UTF-32LE BOM `FF FE 00 00` to UTF-16LE; §2.10.2 "mixed/invalid → fail clearly").
    #[test]
    fn classify_encoding_honours_supported_boms_and_declines_utf32() {
        assert_eq!(
            classify_encoding(b"\xEF\xBB\xBFid,name\n"),
            Some(UTF_8),
            "§2.10.2: a UTF-8 BOM is detected as UTF-8"
        );
        assert_eq!(
            classify_encoding(b"\xFF\xFEi\0d\0"),
            Some(encoding_rs::UTF_16LE),
            "§2.10.2: a UTF-16LE BOM wins over the NUL binary guard (BOM checked first)"
        );
        assert_eq!(
            classify_encoding(b"\xFE\xFF\0i\0d"),
            Some(encoding_rs::UTF_16BE),
            "§2.10.2: a UTF-16BE BOM is detected as UTF-16BE (also over the NUL guard)"
        );
        assert_eq!(
            classify_encoding(b"\xFF\xFE\0\0a\0\0\0"),
            None,
            "§2.10.2: a UTF-32LE BOM is UNSUPPORTED → None, NEVER mis-mapped to UTF-16LE (the for_bom alias trap)"
        );
        assert_eq!(
            classify_encoding(b"\0\0\xFE\xFFabcd"),
            None,
            "§2.10.2: a UTF-32BE BOM is UNSUPPORTED → None"
        );
    }

    // §6.4.1 unit (G15): a BOM-less valid-UTF-8 body is UTF-8 (the §2.10.2 default). Truncation tolerance is
    // gated on FILLING the window: a full-window source whose final byte is a cut multi-byte lead stays UTF-8,
    // but a SHORT source ending in a lone high byte is the real file end (a codepage), not truncated-UTF-8.
    #[test]
    fn classify_encoding_defaults_to_utf8_and_gates_truncation_on_a_full_window() {
        assert_eq!(
            classify_encoding(b"id,name\n1,alpha\n"),
            Some(UTF_8),
            "§2.10.2: BOM-less valid ASCII/UTF-8 is UTF-8"
        );
        // A FULL window (len == MAX_HEADER_WINDOW) whose last byte is 0xC3 (a 2-byte 'é' lead cut by the
        // boundary): the source continues past the window → a genuine truncation → still UTF-8.
        let mut full_window = vec![b'a'; MAX_HEADER_WINDOW - 1];
        full_window.push(0xC3);
        assert_eq!(
            classify_encoding(&full_window),
            Some(UTF_8),
            "§2.10.2/§1.2: a multi-byte char cut at the FULL 4-KiB window boundary stays UTF-8"
        );
        // A SHORT source ("café" in Windows-1252, ending in a lone 0xE9) FITS the window — its trailing
        // incomplete char is the real end, NOT a window cut, so it is a codepage, never a truncated-UTF-8 guess.
        let short_latin1 = classify_encoding(b"caf\xE9");
        assert!(
            short_latin1.is_some_and(|enc| enc != UTF_8),
            "§2.10.2: a short source ending in a lone high byte is a codepage, not a truncated-UTF-8 false positive"
        );
    }

    // §6.4.1 unit (G15): genuinely-invalid-UTF-8 (a lead byte followed by a non-continuation, NOT a boundary
    // truncation) falls to the chardetng single-byte codepage heuristic → a NON-UTF-8 text encoding, and that
    // surfaces a §1.4 encoding_hint (a NON-default encoding is named). Asserts the *class* (Some, non-UTF-8,
    // hinted), not chardetng's exact codepage guess (statistical, not pinned here).
    #[test]
    fn classify_encoding_falls_back_to_a_codepage_for_invalid_utf8() {
        // 0xE9 (Windows-1252 'é') is a UTF-8 3-byte lead; the following space is not a continuation → invalid
        // MID-stream (not a boundary truncation), so the UTF-8 branch declines and chardetng guesses.
        let detected = classify_encoding(b"caf\xE9 latte for the whole office today");
        assert!(
            detected.is_some_and(|enc| enc != UTF_8),
            "§2.10.2: invalid UTF-8 (non-truncation) is classified via the codepage heuristic, not UTF-8"
        );
        assert!(
            detected.and_then(encoding_hint).is_some(),
            "§1.4: a non-default detected encoding surfaces an encoding_hint"
        );
    }

    // §6.4.1 unit (G15): the §1.2 "confirm bytes decode as text" gate — a NUL byte (no BOM) means binary, so
    // classification declines (None → the dispatcher's Uncertain), never a false text guess.
    #[test]
    fn classify_encoding_rejects_binary_with_a_nul_byte() {
        assert_eq!(
            classify_encoding(b"noise\0\x01\x02\x03here"),
            None,
            "§1.2/§2.10.2: a NUL byte (no BOM) is binary, not text — classification declines"
        );
    }

    // §6.4.1 unit (G15): the §1.4 encoding_hint projection — UTF-8 (the §2.10.2 default) needs no hint (None); a
    // non-default encoding is named with its canonical label.
    #[test]
    fn encoding_hint_is_none_for_utf8_and_named_otherwise() {
        assert_eq!(
            encoding_hint(UTF_8),
            None,
            "§1.4/§2.10.2: UTF-8 is the assumed default — no hint"
        );
        assert_eq!(
            encoding_hint(encoding_rs::WINDOWS_1252),
            Some("windows-1252".to_owned()),
            "§1.4: a non-default encoding surfaces its canonical WHATWG name as the hint"
        );
    }

    // §6.4.1 unit (G15): the degenerate empty-slice edge — a DIRECT `classify_encoding(&[])` reads as trivial
    // (vacuously valid) UTF-8. This path is NOT how an empty FILE is classified: `detect` short-circuits a
    // 0-byte header to `DetectionOutcome::Empty` (P3.26) before any text step runs, so the empty-vs-Empty
    // outcome is owned upstream (P3.26 detect / the P3.29 outcome rules), not here. Pinned so the behaviour is
    // explicit rather than an accident.
    #[test]
    fn classify_encoding_on_an_empty_slice_is_trivially_utf8() {
        assert_eq!(
            classify_encoding(&[]),
            Some(UTF_8),
            "§1.2: an empty slice is vacuously valid UTF-8 here; the empty-FILE outcome is detect's Empty (P3.26)"
        );
    }

    // ─── P3.28 — §1.2 CSV-vs-TSV delimiter sniff (spreadsheets.md "Delimiter detection policy") ─────────

    // §6.4.1 unit (G15): the P3.28 headline — a CONSISTENTLY tab-delimited file is TSV **even named `.csv`**
    // (content over name, §1.2/spreadsheets.md). The `.csv` extension hint (Comma) does NOT override a clear
    // content winner — the hint only breaks a genuine TIE, and tab is the sole viable delimiter here.
    #[test]
    fn classify_delimiter_tab_content_beats_a_csv_extension() {
        let tsv = sniff_delimiter(
            "a\tb\tc\nd\te\tf\ng\th\ti",
            Some(ExtensionDelimiterHint::Comma),
        );
        assert_eq!(
            tsv,
            DelimiterClass::Detected(Delimiter::Tab),
            "§1.2/spreadsheets.md: a consistent tab file is TSV even with a .csv extension hint (content over name)"
        );
        assert_eq!(
            tsv.user_facing_format(),
            Some(UserFacingFormat::Tsv),
            "§1.3: a tab winner groups as TSV (CSV ≠ TSV, delimiter-determined)"
        );
    }

    // §6.4.1 unit (G15): a consistent comma file is CSV; comma is the default → no delimiter_hint.
    #[test]
    fn classify_delimiter_comma_is_csv_with_no_hint() {
        let csv = sniff_delimiter("id,name,city\n1,alpha,berlin\n2,beta,munich", None);
        assert_eq!(csv, DelimiterClass::Detected(Delimiter::Comma));
        assert_eq!(csv.user_facing_format(), Some(UserFacingFormat::Csv));
        assert_eq!(
            delimiter_hint(csv),
            None,
            "§1.4: comma is the CSV default — no delimiter_hint (mirrors encoding_hint's None-for-UTF-8)"
        );
    }

    // §6.4.1 unit (G15): the §2.10.2 semicolon-CSV property — a European-Excel `;`-separated file with a
    // `1,50` DECIMAL comma is detected as CSV on the SEMICOLON (not mis-split on the literal comma inside
    // `1,50`), and the non-obvious `;` surfaces as the delimiter_hint. The header line `name;price;note`
    // carries no comma, so comma's per-line consistency breaks and semicolon wins by agreement.
    #[test]
    fn classify_delimiter_semicolon_csv_is_not_missplit_on_decimal_comma() {
        let semi = sniff_delimiter("name;price;note\nfoo;1,50;cheap\nbar;2,30;fair", None);
        assert_eq!(
            semi,
            DelimiterClass::Detected(Delimiter::Semicolon),
            "§2.10.2: a ;-separated file with a 1,50 decimal is CSV-on-semicolon, not mis-split on the comma"
        );
        assert_eq!(semi.user_facing_format(), Some(UserFacingFormat::Csv));
        assert_eq!(
            delimiter_hint(semi),
            Some(";".to_owned()),
            "§1.4/spreadsheets.md: the non-obvious semicolon delimiter is surfaced in the summary"
        );
    }

    // §6.4.1 unit (G15): a pipe-delimited file is CSV with the pipe surfaced as the hint.
    #[test]
    fn classify_delimiter_pipe_is_csv_with_a_pipe_hint() {
        let pipe = sniff_delimiter("a|b|c\nd|e|f\ng|h|i", None);
        assert_eq!(pipe, DelimiterClass::Detected(Delimiter::Pipe));
        assert_eq!(pipe.user_facing_format(), Some(UserFacingFormat::Csv));
        assert_eq!(delimiter_hint(pipe), Some("|".to_owned()));
    }

    // §6.4.1 unit (G15): NO consistent delimiter (single-column text / prose) → Ambiguous → the dispatcher's
    // Uncertain — a clear decline, NEVER an extension fallback (a .csv hint does NOT rescue it to comma).
    #[test]
    fn classify_delimiter_single_column_is_ambiguous_never_extension_fallback() {
        let prose = sniff_delimiter(
            "alpha\nbeta\ngamma\ndelta",
            Some(ExtensionDelimiterHint::Comma),
        );
        assert_eq!(
            prose,
            DelimiterClass::Ambiguous,
            "§1.2/SSOT: no consistent delimiter → Uncertain, never extension-fall-back to comma"
        );
        assert_eq!(
            prose.user_facing_format(),
            None,
            "§1.3: an ambiguous sniff yields no grouping format"
        );
        assert_eq!(delimiter_hint(prose), None);
    }

    // §6.4.1 unit (G15): empty and whitespace-only text carry no non-empty records → Ambiguous (no false guess).
    #[test]
    fn classify_delimiter_empty_and_blank_are_ambiguous() {
        assert_eq!(sniff_delimiter("", None), DelimiterClass::Ambiguous);
        assert_eq!(
            sniff_delimiter("   \n  \n    ", None),
            DelimiterClass::Ambiguous,
            "§1.2: whitespace-only text has no non-empty records → Ambiguous, not a false guess"
        );
    }

    // §6.4.1 unit (G15): a GENUINE tie — every line is consistently splittable by BOTH comma and tab (each
    // yields 2 fields) — is broken by the §1.2 tie-break: the extension hint decides (.csv → CSV, .tsv →
    // TSV), and with NO hint the "then comma" secondary tie-break yields CSV (comma-first).
    #[test]
    fn classify_delimiter_genuine_tie_is_broken_by_the_extension_then_comma() {
        let tie = "a,b\tc\nd,e\tf\ng,h\ti"; // comma → 2 fields/line; tab → 2 fields/line (both consistent)
        assert_eq!(
            sniff_delimiter(tie, Some(ExtensionDelimiterHint::Comma)),
            DelimiterClass::Detected(Delimiter::Comma),
            "spreadsheets.md: a .csv extension breaks the comma/tab tie toward comma → CSV"
        );
        assert_eq!(
            sniff_delimiter(tie, Some(ExtensionDelimiterHint::Tab)),
            DelimiterClass::Detected(Delimiter::Tab),
            "spreadsheets.md: a .tsv extension breaks the same tie toward tab → TSV (content-consistent both ways)"
        );
        assert_eq!(
            sniff_delimiter(tie, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "spreadsheets.md: with no extension hint the tie-break falls to comma (comma-first CANDIDATES order)"
        );
    }

    // §6.4.1 unit (G15): RFC-4180 quote-awareness — a delimiter INSIDE a "…" field is literal and does not
    // count, so a tab-delimited file whose first field is a quoted string CONTAINING commas is still TSV (the
    // commas are quoted; the tab is the real separator). The §2.10.2 "not mis-split" guard at the sniff layer.
    #[test]
    fn classify_delimiter_is_quote_aware() {
        let quoted = "\"a,b,c\"\tx\n\"d,e,f\"\ty"; // 3 commas inside quotes/line; 1 real tab/line
        assert_eq!(
            sniff_delimiter(quoted, None),
            DelimiterClass::Detected(Delimiter::Tab),
            "RFC-4180: commas inside a quoted field are literal — the tab is the delimiter → TSV"
        );
    }

    // §6.4.1 unit (G15): a quoted field spanning an EMBEDDED NEWLINE does not split a record — the two
    // physical lines are one logical record, so a comma file with an embedded newline still sniffs as CSV.
    #[test]
    fn classify_delimiter_handles_an_embedded_quoted_newline() {
        // record 1: `1,"multi\nline",x` (2 commas, one embedded newline inside quotes); record 2: `2,plain,y`.
        let embedded = "1,\"multi\nline\",x\n2,plain,y";
        assert_eq!(
            sniff_delimiter(embedded, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "RFC-4180: a newline inside a quoted field does not end the record — still comma-CSV"
        );
    }

    // §6.4.1 unit (G15): ragged rows (uneven field counts, spreadsheets.md edge case) do not defeat the
    // sniff — a comma file where MOST lines agree on the field count still classifies as comma-CSV via the
    // majority rule, even with one short row.
    #[test]
    fn classify_delimiter_tolerates_a_ragged_row() {
        // three 3-field lines + one short 2-field line: comma count [2,2,2,1] → modal 2 on 3 of 4 → majority.
        let ragged = "a,b,c\nd,e,f\ng,h,i\nj,k";
        assert_eq!(
            sniff_delimiter(ragged, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "spreadsheets.md: a ragged short row does not defeat the majority-consistency comma sniff"
        );
    }

    // §6.4.1 unit (G15): the §1.4 delimiter_hint projection — None for the canonical defaults (comma on CSV,
    // tab on TSV), Some for the non-obvious semicolon / pipe.
    #[test]
    fn delimiter_hint_names_only_the_non_obvious_delimiters() {
        assert_eq!(
            delimiter_hint(DelimiterClass::Detected(Delimiter::Comma)),
            None
        );
        assert_eq!(
            delimiter_hint(DelimiterClass::Detected(Delimiter::Tab)),
            None
        );
        assert_eq!(
            delimiter_hint(DelimiterClass::Detected(Delimiter::Semicolon)),
            Some(";".to_owned())
        );
        assert_eq!(
            delimiter_hint(DelimiterClass::Detected(Delimiter::Pipe)),
            Some("|".to_owned())
        );
        assert_eq!(delimiter_hint(DelimiterClass::Ambiguous), None);
    }

    // §6.4.1 unit (G15): the §1.3 grouping-key mapping — tab ⇒ TSV, comma/semicolon/pipe ⇒ CSV, ambiguous ⇒ None.
    #[test]
    fn delimiter_class_maps_to_the_grouping_format() {
        assert_eq!(
            DelimiterClass::Detected(Delimiter::Tab).user_facing_format(),
            Some(UserFacingFormat::Tsv)
        );
        assert_eq!(
            DelimiterClass::Detected(Delimiter::Comma).user_facing_format(),
            Some(UserFacingFormat::Csv)
        );
        assert_eq!(
            DelimiterClass::Detected(Delimiter::Semicolon).user_facing_format(),
            Some(UserFacingFormat::Csv)
        );
        assert_eq!(
            DelimiterClass::Detected(Delimiter::Pipe).user_facing_format(),
            Some(UserFacingFormat::Csv)
        );
        assert_eq!(DelimiterClass::Ambiguous.user_facing_format(), None);
    }

    // §6.4.1 unit (G15): the extension tie-break hint derives case-insensitively from the spreadsheets.md
    // CSV/TSV extensions (.csv → Comma, .tsv/.tab → Tab), None otherwise — the ONLY place the extension
    // enters delimiter detection, and only as a tie-break input.
    #[test]
    fn extension_delimiter_hint_from_extension() {
        assert_eq!(
            ExtensionDelimiterHint::from_extension("csv"),
            Some(ExtensionDelimiterHint::Comma)
        );
        assert_eq!(
            ExtensionDelimiterHint::from_extension("CSV"),
            Some(ExtensionDelimiterHint::Comma),
            "the extension hint is case-insensitive"
        );
        assert_eq!(
            ExtensionDelimiterHint::from_extension("tsv"),
            Some(ExtensionDelimiterHint::Tab)
        );
        assert_eq!(
            ExtensionDelimiterHint::from_extension("tab"),
            Some(ExtensionDelimiterHint::Tab)
        );
        assert_eq!(ExtensionDelimiterHint::from_extension("txt"), None);
        assert_eq!(ExtensionDelimiterHint::from_extension(""), None);
    }

    // §6.4.1 unit (G15): the byte-level entry point decodes through the P3.27 encoding before sniffing — a
    // UTF-8 comma file classifies as CSV.
    #[test]
    fn classify_delimiter_decodes_utf8_bytes() {
        let csv = classify_delimiter(b"a,b,c\nd,e,f", UTF_8, None);
        assert_eq!(csv, DelimiterClass::Detected(Delimiter::Comma));
    }

    // §6.4.1 unit (G15): the decode is REQUIRED, not cosmetic — in UTF-16LE a comma is the two bytes `2C 00`,
    // so a byte-level sniff would miscount. `classify_delimiter` decodes via the detected encoding first, so a
    // UTF-16LE comma file still classifies as CSV. (This is why the sniff takes bytes + encoding, not text.)
    #[test]
    fn classify_delimiter_decodes_utf16le_bytes() {
        let utf16le: Vec<u8> = "a,b,c\nd,e,f"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            classify_delimiter(&utf16le, encoding_rs::UTF_16LE, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "§1.2: the ASCII delimiter is counted correctly only after decoding UTF-16 to text"
        );
    }

    // §6.4.1 unit (G15): the counting inverse `candidate_index` stays in lock-step with the `CANDIDATES`
    // declaration order — `candidate_index(CANDIDATES[i].as_char()) == Some(i)` for every candidate — so a
    // future reorder of CANDIDATES can never silently mis-address a RecordCounts slot.
    #[test]
    fn candidate_index_matches_candidates_order() {
        for (index, candidate) in Delimiter::CANDIDATES.into_iter().enumerate() {
            assert_eq!(
                candidate_index(candidate.as_char()),
                Some(index),
                "candidate_index must map each CANDIDATES char back to its slot index"
            );
        }
        assert_eq!(
            candidate_index('x'),
            None,
            "a non-delimiter char occupies no slot"
        );
    }

    // §6.4.1 unit (G15): RFC-4180 escaped quotes (`""` inside a quoted field) do not toggle the quote state,
    // so a comma file whose first field contains an escaped quote (`"a""b"`) is still comma-CSV (the `""` is
    // a literal quote character, the following comma is the real delimiter).
    #[test]
    fn classify_delimiter_handles_escaped_quotes() {
        let escaped = "\"a\"\"b\",c\n\"d\"\"e\",f"; // field `a"b` then `c`; field `d"e` then `f`
        assert_eq!(
            sniff_delimiter(escaped, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "RFC-4180: an escaped \"\" stays inside the quoted field — the real comma is the delimiter → CSV"
        );
    }

    // §6.4.1 unit (G15): CRLF line endings — the `\r` of a `\r\n` is ignored (not content, not a delimiter),
    // so a Windows-authored comma file with CRLF endings still classifies as comma-CSV.
    #[test]
    fn classify_delimiter_handles_crlf_line_endings() {
        let crlf = "id,name\r\n1,alpha\r\n2,beta\r\n";
        assert_eq!(
            sniff_delimiter(crlf, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "§1.2: CRLF endings do not disturb the delimiter sniff — the \\r is ignored"
        );
    }

    // §6.4.1 unit (G15) / §04 spreadsheets "CRLF/LF/CR normalised on read" (P3.75 sweep): a lone-CR (classic-Mac)
    // CSV must sniff its delimiter — before the fix the `\r` arm was a pure no-op, so a CR-delimited file
    // collapsed to ONE sampled record, failed the viable-agreement floor, and was wrongly declined `Uncertain`.
    // Now a lone CR closes the record (the csv-crate transform already normalises CR on read, so a detected
    // CR-CSV converts cleanly — detection was the only gap).
    #[test]
    fn classify_delimiter_handles_lone_cr_line_endings() {
        let cr_only = "id,name\r1,alpha\r2,beta\r";
        assert_eq!(
            sniff_delimiter(cr_only, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "§1.2: a lone CR is a record terminator — a CR-delimited CSV is detected, not declined Uncertain"
        );
    }

    // §6.4.1 unit (G15): the sample is bounded to DELIMITER_SNIFF_SAMPLE_LINES records — a file with far more
    // lines than the cap still classifies from the bounded sample (the sniff never walks the whole file, the
    // §1.2/§2.12.4 bounded-read property), and the majority rule holds over the capped window.
    #[test]
    fn classify_delimiter_samples_only_the_bounded_leading_records() {
        let many_lines: String = (0..DELIMITER_SNIFF_SAMPLE_LINES + 5)
            .map(|row| format!("a{row},b{row},c{row}\n"))
            .collect();
        assert_eq!(
            sniff_delimiter(&many_lines, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "§1.2/§2.12.4: a >sample-cap file classifies from the bounded leading records, comma-CSV"
        );
    }

    // §6.4.1 unit (G15): a single PROSE line with one INCIDENTAL delimiter is NOT CSV — consistency is a
    // cross-line property, so one comma in one sentence declines to Ambiguous (§1.2/spreadsheets.md "ambiguous
    // → decline clearly … never a wrong split"), never a false CSV off a lone unverifiable observation. The
    // `.csv` extension hint cannot rescue it (never silently extension-fall-back).
    #[test]
    fn classify_delimiter_prose_with_an_incidental_delimiter_is_ambiguous() {
        assert_eq!(
            sniff_delimiter("Hello, world", Some(ExtensionDelimiterHint::Comma)),
            DelimiterClass::Ambiguous,
            "§1.2: one incidental comma in a single prose line is not a consistent split → Ambiguous, not CSV"
        );
        // A single line of genuine delimited text is still one unverifiable observation → decline (need ≥ 2
        // agreeing records to establish consistency).
        assert_eq!(
            sniff_delimiter("a,b,c", None),
            DelimiterClass::Ambiguous,
            "§1.2: a lone line cannot establish a consistent field-count across the sample → Ambiguous"
        );
        // Two records where the delimiter appears in only one (count [1, 0]) is not a strict majority → decline.
        assert_eq!(
            sniff_delimiter("Hello, world\nGoodbye", None),
            DelimiterClass::Ambiguous,
            "§1.2: a delimiter present in only some rows is inconsistent → Ambiguous, not CSV"
        );
    }

    // §6.4.1 unit (G15): RFC-4180 field-position-aware quoting — a bare `"` MID-FIELD (an inch mark `5' 10"`,
    // an informal quote) is a literal character, NOT a quote opener, so it does not spuriously swallow the
    // following `\n`/delimiter and merge records. A comma file carrying mid-field inch marks stays comma-CSV.
    #[test]
    fn classify_delimiter_treats_a_mid_field_quote_as_literal() {
        let inch_marks = "Alice,5' 10\" tall\nBob,6' 1\" short";
        assert_eq!(
            sniff_delimiter(inch_marks, None),
            DelimiterClass::Detected(Delimiter::Comma),
            "RFC-4180: a non-field-initial \" is literal — the records stay split and the comma is the delimiter"
        );
    }

    // ─── P3.29 — detect() text-classification wiring + §1.2 outcome rules ───────────────────────────────

    // §6.4.1 unit (G15): P3.29 wires detect's step 3 — a consistent comma text body is Recognized as CSV with
    // High confidence and no dims (non-raster), produced by content alone (detect is extension-free).
    #[test]
    fn detect_recognizes_csv_text_as_recognized_csv_high() {
        assert_eq!(
            detect(b"id,name,city\n1,alpha,berlin\n2,beta,munich"),
            DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
            "§1.2/P3.29: a consistent comma text body detects as CSV (High confidence, non-raster dims None)"
        );
    }

    // §6.4.1 unit (G15): a consistent tab body is Recognized as TSV — content over name (detect sees no
    // extension, so the tab content alone determines TSV; a `.csv`-named tab file would detect identically).
    #[test]
    fn detect_recognizes_tab_text_as_recognized_tsv() {
        assert_eq!(
            detect(b"a\tb\tc\nd\te\tf\ng\th\ti"),
            DetectionOutcome::Recognized {
                format: UserFacingFormat::Tsv,
                confidence: Confidence::High,
                dims: None,
            },
            "§1.2/P3.29: a consistent tab body detects as TSV — delimiter-determined, content over name"
        );
    }

    // §6.4.1 unit (G15): a semicolon-CSV (European Excel) is Recognized as CSV (a semicolon winner → CSV, P3.28).
    #[test]
    fn detect_recognizes_semicolon_csv_as_csv() {
        assert_eq!(
            detect(b"name;price;note\nfoo;1,50;cheap\nbar;2,30;fair"),
            DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
            "§1.2/P3.29: a consistent semicolon body detects as CSV"
        );
    }

    // §6.4.1 unit (G15): confirmed TEXT with NO consistent delimiter (prose) is Uncertain — text but not a
    // supported P3 delimited format — never a false CSV/TSV and never extension-fallback (§1.2 outcome rules).
    #[test]
    fn detect_maps_non_delimited_text_to_uncertain() {
        assert_eq!(
            detect(b"the quick brown fox jumps over the lazy dog\nand keeps on running here"),
            DetectionOutcome::Uncertain { best_guess: None },
            "§1.2/P3.29: text with no consistent delimiter is Uncertain, not a false CSV/TSV guess"
        );
    }
}

// ─── P3.30 — the §1.2 detection KAT (tests/detect-kat.toml), machine-enforced at L2 (G15) ────────────
//
// The Known-Answer-Test reader is `#[cfg(test)]`-only: it reads the committed `tests/detect-kat.toml` +
// each `tests/corpus/` fixture it pins and asserts `detect()` reproduces the pinned §1.2 outcome, turning
// §6.4.1's prose claim into an L2 tripwire — a detector refactor that silently mis-classifies an
// ambiguous/misnamed signature fails here (a fast unit test), not only at the L4 corpus. Both files live at
// the WORKSPACE root (`../tests/` from this crate's `src-tauri/` manifest dir). The reader stays index-free
// under the module `indexing_slicing` deny (`.get`/`split_once`/`find`, no `[]`), and surfaces a missing
// fixture as a test failure rather than through `panic!`, which has no test exception in this crate.
// [Build-Session-Entscheidung: P3.30]
#[cfg(test)]
mod kat_tests {
    use super::{detect, read_header};
    use crate::domain::{DetectionOutcome, UserFacingFormat};
    // [Build-Session-Entscheidung: P3.61] The corpus/`tests/` path resolution moved to the §6.4.5
    // SINGLE-SOURCE helper (`crate::test_corpus`): P3.61's sentinel test in `crate::engines` needs the same
    // resolution, and re-deriving `CARGO_MANIFEST_DIR/../tests` there would be the inline duplication
    // test-strategy §3 (:601) forbids. This module's own `fn tests_dir()` was that single source while it was
    // the only consumer.
    use crate::test_corpus::{corpus_dir, tests_dir};

    /// One pinned KAT case: the corpus-relative fixture `file` and its `expect` (a FormatId or an outcome name).
    struct KatCase {
        file: String,
        expect: String,
    }

    /// Parse the `[[case]]` blocks of `detect-kat.toml` — a minimal reader over the KAT's controlled subset
    /// (non-comment `[[case]]` headers + `key = "value"` lines), collecting `file` + `expect` per case. It
    /// deliberately pulls in NO general TOML parser (no `toml` dev-dep — which would need a §0.8 floor row):
    /// the KAT is a small hand-authored format, so a purpose-built line reader keeps the test self-contained.
    /// Comment lines (`#…`, incl. the reference template) contribute no case; a case missing either field is
    /// dropped (the count/coverage asserts then flag the loss).
    fn parse_cases(text: &str) -> Vec<KatCase> {
        let mut cases: Vec<KatCase> = Vec::new();
        let mut file: Option<String> = None;
        let mut expect: Option<String> = None;
        let mut in_case = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[[case]]" {
                if let (Some(f), Some(e)) = (file.take(), expect.take()) {
                    cases.push(KatCase { file: f, expect: e });
                }
                in_case = true;
                continue;
            }
            if !in_case {
                continue;
            }
            if let Some((key, value)) = parse_kv(line) {
                match key {
                    "file" => file = Some(value),
                    "expect" => expect = Some(value),
                    _ => {}
                }
            }
        }
        if let (Some(f), Some(e)) = (file.take(), expect.take()) {
            cases.push(KatCase { file: f, expect: e });
        }
        cases
    }

    /// `key = "value"` → `(key, value)` for a KAT line: the value is the content between the first and last
    /// double-quote after the `=`. `None` for a non-`key = "value"` line. Index-free (`split_once`/`find`/
    /// `rfind`/`.get`).
    fn parse_kv(line: &str) -> Option<(&str, String)> {
        let (key, rest) = line.split_once('=')?;
        let open = rest.find('"')?;
        let after = rest.get(open + 1..)?;
        let close = after.rfind('"')?;
        let value = after.get(..close)?;
        Some((key.trim(), value.to_owned()))
    }

    /// Resolve a KAT `expect` PascalCase FormatId (`"Csv"`) to a `UserFacingFormat` via serde — the §0.6
    /// wire form is camelCase (`"csv"`), so lowercasing the leading char yields the serde token (`"ThreeGp"`
    /// → `"threeGp"`). `None` when `expect` is an outcome name (`"Uncertain"` / …), which then resolves
    /// against the `DetectionOutcome` set — FormatId FIRST, the disjoint-sets resolution order the KAT
    /// convention mandates.
    fn format_of(expect: &str) -> Option<UserFacingFormat> {
        let mut chars = expect.chars();
        let camel: String = chars
            .next()
            .map(|c| c.to_ascii_lowercase())
            .into_iter()
            .chain(chars)
            .collect();
        serde_json::from_str::<UserFacingFormat>(&format!("\"{camel}\"")).ok()
    }

    /// Does `outcome` satisfy the KAT `expect`? A FormatId expect requires `Recognized { format == that }`
    /// (the KAT pins the FormatId, not the confidence/dims); an outcome-name expect requires that variant.
    fn matches_expect(expect: &str, outcome: &DetectionOutcome) -> bool {
        if let Some(fmt) = format_of(expect) {
            return matches!(outcome, DetectionOutcome::Recognized { format, .. } if *format == fmt);
        }
        match expect {
            "Uncertain" => matches!(outcome, DetectionOutcome::Uncertain { .. }),
            "UnsupportedType" => matches!(outcome, DetectionOutcome::UnsupportedType { .. }),
            "Empty" => matches!(outcome, DetectionOutcome::Empty),
            "Unreadable" => matches!(outcome, DetectionOutcome::Unreadable { .. }),
            _ => false,
        }
    }

    /// Read `detect-kat.toml` from the workspace-root `tests/` dir (a committed test asset).
    fn read_kat() -> String {
        let path = tests_dir().join("detect-kat.toml");
        std::fs::read_to_string(&path).expect("the detect-kat.toml KAT is a committed test asset")
    }

    // §6.4.1 unit (G15): the §1.2 detection KAT — every `tests/detect-kat.toml` `[[case]]` pins a corpus
    // fixture to its exact detection result; assert `detect(fixture)` reproduces it, so a detector refactor
    // that silently mis-classifies an ambiguous/misnamed signature is caught at L2, not only at the L4 corpus.
    #[test]
    fn detect_kat_cases_all_hold() {
        let cases = parse_cases(&read_kat());
        assert!(
            !cases.is_empty(),
            "§6.4.1: the detection KAT must pin at least one case (an empty KAT is a no-op tripwire)"
        );
        let corpus = corpus_dir();
        for case in &cases {
            let fixture = corpus.join(&case.file);
            let read = std::fs::read(&fixture);
            assert!(
                read.is_ok(),
                "§6.4.1 KAT: fixture `{}` must be a committed tests/corpus/ file ({:?})",
                case.file,
                read.as_ref().err()
            );
            let bytes = read.expect("the fixture read is Ok per the assert above");
            let header = read_header(bytes.as_slice()).expect("reading a byte slice never fails");
            let outcome = detect(&header);
            assert!(
                matches_expect(&case.expect, &outcome),
                "§1.2 KAT: detect(`{}`) = {outcome:?}, but the KAT pins `{}` (a silent mis-detection is a \
                 no-misroute regression, §2)",
                case.file,
                case.expect
            );
        }
    }

    // §6.4.1 unit (G15): the KAT is ARMED — the P3.30 walking-skeleton cases are present and every `expect`
    // resolves (a FormatId via serde, or a known outcome name), so the tripwire is not a silent no-op and a
    // future dangling `expect` token is caught here.
    #[test]
    fn detect_kat_covers_the_walking_skeleton_and_the_expect_vocabulary() {
        let cases = parse_cases(&read_kat());
        let expects: Vec<&str> = cases.iter().map(|c| c.expect.as_str()).collect();
        assert!(
            expects.contains(&"Csv") && expects.contains(&"Tsv") && expects.contains(&"Uncertain"),
            "§1.2/P3.30: the KAT pins the walking-skeleton CSV, TSV and Uncertain cases (found {expects:?})"
        );
        for case in &cases {
            assert!(
                format_of(&case.expect).is_some()
                    || matches!(
                        case.expect.as_str(),
                        "Uncertain" | "UnsupportedType" | "Empty" | "Unreadable"
                    ),
                "§6.4.1: KAT expect `{}` resolves to neither a FormatId nor a DetectionOutcome name",
                case.expect
            );
        }
    }
}
