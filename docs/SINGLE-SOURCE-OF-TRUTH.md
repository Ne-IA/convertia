# ConvertIA — Single Source of Truth

> This document captures **the idea** for ConvertIA: what it is, who it's for,
> the rules it lives by, and what it should do. It is intentionally **free of
> technical decisions** — framework, engine packaging, the exact
> format-to-engine mapping and the build pipeline are decided later in the
> **Specification**. This file is the source the Specification is written from,
> and the product-level promises here are ones the Specification must keep.
> Cross-references use section *names* (not numbers) so they survive edits.

## 1. The Idea in One Sentence

A portable, install-free desktop app that lets anyone convert common everyday
files into other sensible formats by dragging them onto a single drop area.
*(This line is the vision — what v1 actually covers is bound by the **v1
Definition of Done**, which is "everything in *What It Converts*.")*

## 2. License & Openness

- **MIT licensed.** Free as in freeware *and* free as in open source.
- **Copyright holder.** The `LICENSE` header and `NOTICE` read
  `Copyright (c) 2026 Ne-IA and ConvertIA contributors` (2026 = inception year).
  "Ne-IA" is the brand under which the project's owner publishes. Contributors
  **retain copyright** in their contributions (no assignment, no CLA); authorship
  is recorded in Git history (DCO). The collective notice form is used precisely
  because inbound = outbound with no assignment.
- **Public repository** under the Ne-IA organization. First fully-open product.
- **Provided as-is (no warranty).** Per MIT, ConvertIA comes with no warranty. It
  never harms your originals, but you are responsible for checking that a
  converted result fits your needs; the project accepts no liability for the
  *contents* of converted output. This is surfaced in the About screen.
- **Contributions** are welcome and licensed inbound under MIT (inbound =
  outbound), with **no CLA** (a DCO sign-off may be requested, not required).
  Contributors warrant their submissions are their own work or compatibly
  licensed for inbound MIT; code from incompatibly-licensed sources is not
  accepted. A `CONTRIBUTING` note, a code of conduct, a `SECURITY` policy
  (private vulnerability reporting), and a plain-language `PRIVACY` statement
  accompany the repo.
- **Engine-license policy.** ConvertIA's own code is MIT. Its value depends on
  third-party conversion engines that carry their own licenses, **all bundled
  inside the build** (nothing is fetched at runtime). Policy: *nothing ships that
  compromises the open license* — and "compatible terms" **explicitly includes
  copyleft**: where a bundled engine is GPL/LGPL or similar, it ships as a
  **separate, independently-invoked binary** (aggregation, not static linking
  into the MIT core), its obligations are honored (license text plus a written
  offer of source where required), and ConvertIA's own code stays cleanly MIT.
  Every engine's license/notice is surfaced (a `NOTICE` / third-party-licenses
  file, via the SBOM). As a **best-effort maintenance posture** (not a v1 ship
  gate, no SLA, no committed patch turnaround) bundled engines are kept
  reasonably current/patched — they are third-party decoders and a classic
  attack surface, so "bundle once, never update" is not acceptable. Some everyday
  formats — notably **HEIC** and **AAC** — are patent-encumbered and a known
  open-source distribution grey area; whether to ship, gate, or rely on the OS
  for such a format is an explicit decision the Specification must make (see *v1
  Definition of Done* for the patent-related exception this creates).
- **Security posture.** ConvertIA opens arbitrary, possibly malicious files
  through those third-party decoders. Intent: decoding untrusted input is
  isolated/contained so a decoder crash or hang fails that one item clearly
  (per *Fail clearly*) without wedging the app or compromising the no-harm
  guarantee. The isolation/sandboxing mechanism is a Spec matter; the *intent*
  is stated here so the Spec does not treat it as optional.
- **Trademark.** The MIT grant covers the **code, not the "ConvertIA" name or the
  Ne-IA logo.** Forks and redistributions must use a different name and may not
  use the Ne-IA logo (guidelines in `TRADEMARK.md`).
- **Naming.** "ConvertIA" follows the Ne-IA family. Trademark / name-collision
  risk for **both** "ConvertIA" and the public use of the "Ne-IA" brand has
  **not** yet been cleared; a clearance check (in the jurisdictions relevant to
  a globally-downloadable app) is a precondition before first public release,
  and the name may change if a conflict is found.
- **Distribution & download trust.** Releases are published from **one canonical
  location** — the Ne-IA org's GitHub Releases — as the single source of
  authentic builds; each release ships published **integrity hashes** (plus the
  SBOM) so a download can be verified, and the download page states the
  as-is/no-warranty + best-effort-security posture and how to verify the hash
  (reaching the user at the highest-risk moment). Because signing/notarization is
  deliberately out of scope (see *Out of Scope*), this
  published-checksum-from-a-canonical-source approach is the stated trust
  substitute.

## 3. Who It's For

The everyday person — students, office workers, hobbyists, anyone who just
needs a file in another format without hunting for sketchy online converters.

**Not** for specialists. The single inclusion test lives in **What It Converts**.

## 4. Principles (the rules it lives by)

1. **Completeness within scope beats lightweight.** Stay as light as reasonably
   possible, but for a conversion that is *in scope*, prefer completeness over a
   smaller footprint. The full in-scope coverage ships in v1 (see *v1 Definition
   of Done*); this principle decides quality/coverage, never a reason to defer a
   listed format.
2. **Portable, no installation.** Download, run, done. No installer, no admin
   rights, no system pollution.
3. **Cross-platform, one product.** A single codebase producing **one artifact
   per platform** (Windows, macOS, Linux) — three builds, one product, not three
   separate apps. *One product does not mean identical feature availability
   everywhere* — a few patent-encumbered conversions may be honestly unavailable
   on a given platform (per the engine-license policy); the product, UX and
   guarantees are otherwise identical.
4. **Local, private & offline.** Conversions happen on the user's machine; user
   files are **never uploaded anywhere**, there are no accounts and no telemetry.
   The app is **fully self-contained and works completely offline**: every
   in-scope conversion ships inside the build and runs with **zero network
   access** — nothing is ever downloaded after the app itself. "Private" means
   *nothing leaves the machine over the network as a result of what ConvertIA
   itself does* — it cannot control other software you run: converting files
   inside a cloud-synced or shared folder (OneDrive, iCloud, Dropbox, a corporate
   share) means your own sync tool may upload the originals and the results;
   ConvertIA neither causes, prevents, nor detects that (also noted in About).
   ConvertIA does **not** check for updates or phone home; users learn of new
   releases by visiting the canonical GitHub Releases page, and any future update
   check would be opt-in and disclosed, never silent. The only network activity
   is **user-initiated** (e.g. opening the project page). There is no silent
   network call.
5. **Never harm the original.** Source files are never overwritten or deleted —
   **including** when source and target format are the same (e.g. re-compressing
   a JPG): the original is kept and the result gets an adapted name. An output
   **keeps the source's base name and takes the target format's extension**
   (`vacation.heic` → `vacation.jpg`); no-clobber numbering only appends `(1)`,
   `(2)`… before that extension — the base name is never replaced, hashed, or
   decorated with words like `_converted`. The **no-clobber guarantee is
   absolute** and is evaluated on the **resolved real file, not the path
   string**: the final write is exclusive (create-new-or-fail), so even if the
   chosen name becomes taken between picking and writing — a concurrent
   conversion, a second app instance, a file that appeared meanwhile — ConvertIA
   picks again rather than overwriting. ConvertIA never writes to, through, or as
   a target that resolves (via symlink, alias, junction or hardlink) onto any
   source in the frozen set; if writing beside a source would resolve onto the
   original, it diverts (per the unwritable-location fallback) rather than risk
   it (the frozen set is de-duplicated by resolved identity, so a file reached
   via two paths is converted once). A conversion **either fully succeeds or
   leaves no file behind**; this holds even across an ungraceful end (crash,
   power loss, force-quit) — the visible output appears atomically, leaving at
   most a discardable temporary artifact (cleaned up on next run; temp artifacts
   are owned per-run so cleanup never removes another instance's in-progress
   file), never a truncated file masquerading as finished. On any failure,
   cancel, or out-of-disk, partial/temporary artifacts are removed so the user's
   free space returns to roughly what it was before the run; if cleanup itself
   can't complete, the item is never reported as a clean success — ConvertIA says
   residue may remain and where. The **source set is frozen at the moment of
   drop/selection**: any file that appears afterward — written by this run, a
   concurrent instance, or anything else — is never ingested as a source in this
   run, and outputs landing in a source folder do not expand or restart the
   batch. Two cases must not be conflated: an ordinary name collision — within a
   run, or against an unrelated pre-existing file — is resolved silently by the
   next-free-variant numbering; but when ConvertIA detects it would re-produce
   output for the **same resolved source + same target + same effective
   settings** (you re-ran the exact same conversion), it does not silently add
   another numbered copy — it shows one plain batch-level prompt (skip as the
   safe default, or make a fresh copy). Any change to target or settings is a new
   conversion using ordinary numbering. This re-run detection is **best-effort**:
   when ConvertIA can't tell (a prior output was renamed/moved, or across
   sessions) it safely falls back to silent next-free-variant numbering, never to
   overwriting. All no-harm, fail-clearly, atomicity, path-limit and free-space
   guarantees apply **identically on the divert/fallback path**. Real-world
   filenames (any language, emoji, spaces, very long paths) are handled without
   mangling; a name whose no-clobber suffix or new extension would exceed the OS
   path limit fails clearly rather than being truncated (truncation is never the
   escape hatch).
6. **Recognize files by content, not the name.** A `.jpg` that is really a PNG,
   or a file with no extension, still works. When detection identifies a real
   but **unsupported** type, ConvertIA says so plainly ("can't convert this type
   — detected: X") rather than showing an empty target list or appearing to
   hang. When detection is **uncertain or conflicting**, it names what it
   believes the file is (or that it can't tell) and declines clearly — it never
   silently falls back to the extension or guesses a target. Detection drives
   both whether a file is eligible *and* how a batch is grouped.
7. **Fail clearly, never cryptically.** A corrupt, empty, 0-byte, unrecognizable
   or out-of-scope file — or a source that was present at drop but is **unreadable
   or gone when its turn comes** (removed media, moved/deleted/renamed file,
   exclusive lock, denied read permission) — produces one plain-language message
   and nothing written for it; the rest of a valid same-format batch keeps going
   (a bad item is skipped mid-run and reported, never silently — this differs
   from the *pre-flight* refusal of a multi-format drop, see *How It Feels*). At
   the end of a batch the user sees a clear summary of what succeeded and what
   failed (and why); a batch where *everything* failed is a clear failure, never
   a quiet finish. An out-of-disk or too-big item fails clearly **and** the batch
   continues. Some conversions are inherently **lossy** (pdf→txt drops layout,
   docx→pdf may reflow, a missing font changes a slide); ConvertIA does its
   honest best and signals predictable loss as a **calm, passive inline note next
   to the chosen target** ("text only — layout and images are dropped") — shown
   only for genuinely predictable loss, never a blocking "I understand" dialog or
   a per-conversion nag. *(This fidelity note is about content faithfulness, not
   downstream compatibility — a valid WEBP/AVIF/OPUS may not open everywhere; the
   pre-highlighted default favors a widely-compatible target.)* No stack traces.
8. **It just works by default.** Every conversion runs to a sensible default
   with **no required choices** — the common path is always *drop → pick a target
   → convert*. Settings are optional refinements, never gates.
9. **Modern, clean UI.** Visually pleasing and contemporary, uncluttered — never
   busy or noisy. A bit of eye candy is welcome. *(This is interface restraint;
   footprint/size is Principle 1.)*
10. **For anyone — accessible.** The drop area is the primary path but not the
    only one: a file picker and the keyboard both work. Basic accessibility
    (keyboard-operable, readable contrast and text sizes) is part of "for
    anyone," not optional polish.
11. **English UI.**

## 5. What It Converts (scope)

The goal is broad, sensible everyday coverage. The formats we cover — as sources
and/or targets; the exact source→target pairs are settled in the Specification —
are:

- **Images** — JPG/JPEG, PNG, WEBP, GIF, BMP, TIFF, **HEIC/HEIF** *(¹)*, AVIF,
  ICO; plus SVG as a *source* (rasterized to PNG/JPG/…).
- **Audio** — MP3, WAV, FLAC, **AAC** *(¹)*, M4A, OGG, OPUS, WMA, AIFF, ALAC.
- **Video** — MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP; plus two
  cross-category outputs: **extract audio** (→ MP3/WAV/…) and **to animated GIF**.
- **Documents** — PDF, DOCX, DOC, ODT, RTF, TXT, MD, HTML (e.g. DOCX→PDF,
  PDF→TXT, MD→PDF/HTML, HTML→PDF).
- **Spreadsheets** — XLSX, XLS, ODS, CSV, TSV (e.g. XLSX↔CSV, XLSX→PDF).
- **Presentations** — PPTX, PPT, ODP, PDF (e.g. PPTX→PDF).

*(¹) Patent-encumbered. Shipped if an openly-redistributable engine exists on a
platform; otherwise honestly surfaced as unavailable there — see the exceptions
in **v1 Definition of Done**.*

**v1 ships the full coverage above**, subject only to the two explicit exceptions
in *v1 Definition of Done* (per-platform patent gaps; last-resort demotion of a
format that cannot meet the reliability bar). The only things otherwise excluded
from v1 are the items under *Future Ideas (Parked)*; everything here is in from
the start (the exact pair matrix is enumerated in the Specification).

**The one inclusion test (canonical).** *Would a normal person plausibly want
this conversion?* If yes, it's a candidate. If only a specialist would
(forensics, lab, exotic specialist-only formats), it's out. **Tie-breaker: when
in doubt, it's out of v1 and parked** in *Future Ideas*. A conversion earns
inclusion by everyday demand, not by being technically possible; the product
intent is the smallest pair set that satisfies everyday demand, not the cartesian
product (within-category exotic pairings like BMP→TIFF are included only if they
pass the same everyday-demand test).

**Direction & shape rule.** v1 favors common-sense, **forward/derivative-direction**
targets (raster→raster, doc→pdf/txt, video→audio/gif). Reverse/reconstructive
conversions (pdf→docx, raster→vector) are out of v1 unless explicitly listed.
Conversions are strictly **one-source → one-target**; one-to-many fan-outs (e.g.
a multi-page PDF → one image per page) are **out of v1** (parked). The only
cross-category outputs in v1 are **extract-audio and to-animated-GIF** — a closed
set; any further cross-category output is a Parked candidate judged by the
inclusion test.

**Content fidelity.** A conversion preserves the source's *content*, not just
its wrapper: text in any language (CJK, right-to-left scripts), mixed encodings,
and CSV encoding/delimiters come through intact, not mangled.

## 6. How It Feels to Use

The idle/first screen is self-explanatory: a clear "drop files here or click to
browse" invitation, a one-line "all conversion happens locally, on your machine"
reassurance, and **no account, setup or configuration** before the first
conversion.

1. **One drop area.** A friendly "drop your files here" zone (clicking it also
   opens a file picker).
2. **Drop files or a whole folder.** A dropped folder is collected recursively
   (subfolders included); hidden/system files (`.DS_Store`, `Thumbs.db`) are
   ignored.
3. **Batch rule (v1): one source format at a time.** Grouping is by the
   **individual user-facing format** (per Principle 6), not the six scope
   categories and not codec-level subtypes: `.jpg` ≠ `.png`, MP4 ≠ MOV, MP3 ≠
   WAV are each distinct and require separate drops; all-JPEG is one
   batch; a multi-category format like PDF is one detected type offered the
   de-duplicated union of its sensible targets. The rule keys **only on the
   source type**; cross-category outputs (extract-audio, to-GIF) are additional
   *targets* of that one source, not a second source. **Before converting,
   ConvertIA shows what it collected** — detected format and count (e.g. "48 JPG
   files") — so the user can confirm the batch, especially for recursively
   collected folders. If a drop or folder contains more than one source format,
   ConvertIA does **not** convert a subset: it names the formats it found and
   asks the user to **re-drop a single format** (a deliberate v1 decision — there
   is no automatic "convert just the JPGs" affordance in v1; mixed-format
   handling is parked). This pre-flight refusal is distinct from skipping a
   single bad file mid-run.
4. **The app reacts to what you dropped.** Based on the detected format it offers
   the sensible **target formats** (drop a `.mov` → mp4, mp3, gif, …). Where one
   target is the obvious everyday choice it is pre-highlighted so the user can
   convert in two clicks (a tie-breaker favors a widely-compatible target, unless
   a modern format like WEBP/AVIF/OPUS is clearly the better everyday choice; the
   Spec fixes the per-source default). **One chosen target applies to the whole
   same-source batch** — per-file target selection is out of v1.
5. **Relevant settings appear contextually.** Important switches (e.g. webp
   quality) are shown directly; many or niche options are tucked behind
   **"Advanced options"** so the default view stays clean. v1 exposes only the
   few settings that materially change a normal user's result — adding a setting
   is a scope change, not a default (rich per-format option sets and remembered
   presets are out of v1).
6. **Visible progress, cancellable.** A real progress bar (not an indeterminate
   spinner) so even a long single conversion reads as *working, not hung*;
   batches process as a queue and can be cancelled — cancelling keeps the files
   already finished and cleanly discards the one in progress (no partial
   leftover, never touches originals). The app stays responsive regardless of
   batch or file size; an item too big to handle, or a run obviously doomed for
   disk space, fails **fast and clearly** (preferably up front) and the rest
   continue.
7. **Output lands somewhere obvious.** The destination is shown and changeable
   **before** conversion starts (a "will save to …" line), not only revealed on
   completion — defaulting to next to each source in place (so folder layout is
   preserved naturally); a user-chosen destination re-creates the relative
   subfolder structure rather than flattening. The fallback is **per-location**:
   a source whose location can't be written (read-only USB, network share,
   restricted folder) — or that sits in a known-ephemeral place the OS may purge
   (a temp dir) — diverts to a single predictable place (Downloads/Documents or a
   folder the user picks), while writable sources still get output beside them.
   Flattened fallback outputs are still de-collided by the no-clobber rule, the
   completion summary maps each output to its source, and "open folder" opens the
   common root of the dropped selection. Existing files are never overwritten;
   ConvertIA never fails silently or aborts the batch.
8. **On completion.** ConvertIA shows where the files went and offers a one-click
   "open folder" / "open file" — the user never has to go hunting.

## 7. Design Intent

- Minimal, modern, a little eye candy — "modern > plain."
- The **Ne-IA logo** appears as branding; a static in-app **About /
  legal-notices** screen is present (credits + third-party-licenses). There is no
  operated service, so no web-style legal-notice obligation applies.
- Logo, colors and final branding are placeholders for now (owner handles them
  separately).

## 8. Explicitly Out of Scope

- Exotic / specialist-only / forensic formats (per the canonical inclusion test
  in *What It Converts*).
- **Platforms** are exactly Windows / macOS / Linux **desktop** — no mobile, web,
  or headless/CLI build in v1.
- **Distribution concerns** — code signing, notarization, store "trust"
  rankings, vendor certificates: not pursued. *Exception:* anything we must
  handle in-code regardless (e.g. generating an SBOM as a build artifact).
- Cloud processing, accounts, telemetry.
- Inbound feature/format requests default to *Future Ideas (Parked)*; only the
  canonical inclusion test promotes them.

## 9. v1 Definition of Done

**No "minimal viable" tiering: v1 ships the full coverage in *What It Converts*.**
There is no time pressure — completeness is the gate, and the only things
deliberately left out of v1 are the items under *Future Ideas (Parked)*. This is
owned as a deliberate trade-off: **v1 is one large, all-or-nothing public
release**, with no fixed deadline for reaching it. Internal *sequencing* of the
work (category by category) is allowed as a Spec/planning concern; partial
*public* release is not.

**Conversions.** The unit of "done" is the individual **source→target pair** (a
category is just its set of pairs). v1 is shippable when:

- every sensible source→target pair across **all** categories in *What It
  Converts* works reliably on **all three platforms** (the exact matrix is
  enumerated in the Specification; the named examples — mov→mp4/mp3/gif,
  png→webp, pdf→txt, docx→pdf, xlsx→csv, pptx→pdf — are illustrations of the bar,
  not a reduced subset);
- "working reliably" = passes the fail-clearly (Principle 7) and no-harm
  (Principle 5) guarantees on a **representative real-world input corpus** (real
  photos, Office docs incl. non-Latin/RTL text, plus representative audio and
  video files). That such a corpus exists is a required v1 asset and a
  precondition for declaring any pair done — so the reliability gate is
  non-circular; its exact contents are a Spec matter;
- **everything runs fully offline** immediately after download — the whole engine
  set is bundled, there is no component fetch;
- **first permissible exception:** a conversion that genuinely cannot be
  distributed under the engine-license policy on a given platform (e.g. a
  patent-encumbered format such as HEIC/AAC with no openly-redistributable
  engine there) is an explicit, documented, honestly-surfaced exception — shown
  as unavailable on that platform, never a convenience cut or a silent omission.
  It is never a reason to defer a format that *can* ship;
- **second permissible exception:** an in-scope, license-clean pair that
  genuinely cannot meet the reliability bar despite reasonable effort may be
  demoted to *Future Ideas (Parked)* as an explicit, documented,
  honestly-surfaced decision (a release-note item) — so one stubborn format can
  never block the whole release forever. Demotion is a last resort, never a
  convenience cut.

**Beyond conversions (also ship gates).** v1 also requires that:

- the offline guarantee (no network activity at all) is observably true;
- **basic accessibility** works (keyboard path + readable contrast/sizes);
- the **core UX flow** works: drag/drop + picker + keyboard reach the same
  result; reacts to detected type; pre-highlighted sensible default; destination
  shown before convert; visible cancellable progress; end-of-batch summary;
  one-click open-folder/file;
- the **unwritable/ephemeral-location fallback** works;
- every bundled engine's **required license text and attribution** is present and
  correct (NOTICE / third-party-licenses, backed by the SBOM) — a missing
  attribution is release-blocking, same status as no-harm;
- **name/trademark clearance** is completed (and any rename applied across repo,
  LICENSE/NOTICE and branding);
- **usability floor:** an ordinary non-technical person can complete each named
  conversion unaided on first try (drop → pick → convert → find output),
  validated by **at least one informal non-developer walkthrough** — on **at
  least one platform** with a genuine non-developer, the owner permitted to run
  the remaining two platform walkthroughs where no non-developer tester is
  available.¹

> ¹ **Owner amendment (intentional, recorded here).** The original wording read
> "at least one informal non-developer walkthrough *per platform*" (three
> platforms). ConvertIA is a solo/hobby project; sourcing a fresh non-developer
> on all three OSes per release is not reliably possible. The SSOT owner amends
> the gate to require **≥1 genuine non-developer walkthrough overall** (the macOS
> Sequoia quarantine step preferentially gets the non-dev tester, being the
> highest non-technical-user blocker), with the **owner** allowed to run the
> remaining platform walkthroughs. The floor's *intent* — a human who did not
> build the app succeeds unaided — is fully preserved by the ≥1 true non-dev pass.
> Spec §6.6 implements this; spec §6.10 DoD row 11 matches it. This is the SSOT
> text being changed, not a spec divergence from it.

**Not a gate.** Subjective visual polish ("modern / eye candy") is iterative,
never release-blocking; engine-currency is a best-effort posture, not a gate.

## 10. Future Ideas (Parked)

Good ideas deliberately **not** in v1, kept so they aren't lost and don't bloat
the first release:

- **Mixed-format / mixed-category batches** in one go (drop HEIC + JPG + … and
  convert all to a single chosen target, or handle each format appropriately,
  including a one-click "convert just the JPGs" affordance at the moment of a
  mixed drop). First enhancement after v1.
- **One-to-many fan-out conversions** (multi-page PDF → one image per page),
  grouped into a clearly named folder.
- **Archive handling** — *extracting* archives (open `.zip`/`.rar`/`.7z`) is the
  strong everyday re-entry candidate (Windows can't natively open `.rar`/`.7z`);
  archive→archive *repacking* (zip→7z) is niche. Doesn't fit the
  one-file→one-format core model, and carries real security surface (zip-slip,
  zip-bombs, symlink / absolute-path) to design deliberately when it returns.
- **E-book conversion** (EPUB / MOBI / AZW3 / FB2 ↔ …) — moderate niche, heavy
  engine (Calibre-class), and a **DRM landmine**: DRM-protected store books can't
  be converted, which would generate "why won't it work?" confusion against the
  "it just works" promise. Revisit deliberately.
- **Font conversion** (ttf / otf / woff / woff2) — leans power-user; revisit.
- **Camera RAW & PSD** (CR2/NEF/ARW → JPG/PNG; PSD → flattened PNG/JPG) —
  photographer / designer territory; revisit by demand.
- **Reverse / reconstructive conversions** (pdf→docx, raster→vector).
- **OCR** (image / scanned PDF → searchable text).
- **UI localization** — the v1 UI is English-only by design (Principle 11);
  translating the interface is parked (file *content* in any language is already
  handled).
- **Batch presets** / remembered per-format settings.
- **Drag-out** of finished results into other apps.

## 11. Boundary Note

Everything technical — framework, how the engines are bundled, the exact
source→target format-to-engine matrix, the precise name-collision and
atomic-write mechanics, decoder isolation/sandboxing, disk-space estimation
thresholds, very-large-batch handling, SBOM tooling, and the build & release
pipeline — is **decided in the Specification, not here.** This file answers
*what & why*; the Spec answers *how*.
