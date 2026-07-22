# P7 — Documents · Spreadsheets · Presentations (office family)

> **Full document/spreadsheet/presentation coverage on the proven P4 harness.**
> "Full coverage" = every enumerated `(source → target)` pair across the three
> office categories is backed by §6.4.5 corpus files + §6.4.3 per-pair integration
> tests and marked **`reliable`** in the §6.5 pair-status ledger on all three
> platforms (or `demoted`/`unavailable` with a recorded `docs/demoted-pairs.md`
> row). Four engines do the work: **LibreOffice headless** (every office↔office +
> every `*→PDF` in the whole platform), **poppler `pdftotext`** (`PDF→TXT`),
> **pandoc** (markup down/up-conversions for the XML/text sources), and the
> **native Rust CSV/TSV engine** (already built end-to-end in P3 — only *broadened*
> here, never rebuilt). P7 stages + hardens LibreOffice/poppler/pandoc, wires every
> office pair through the §2.12 isolation boundary built in P4, registers the
> per-format advanced-option DECLARATIONS against the P4-built options-panel shell,
> and **resolves the LibreOffice 26.2 Markdown-import reliability gate** (the
> load-bearing `MD→PDF` decision — ship via LO Markdown import OR demote `MD→PDF`
> to parked; no chain-free fallback exists, §3.2).
>
> **Spec home:** [`04-formats/documents`](../spec/04-formats/documents.md) (PDF
> canonical home + every doc pair, the DOC→markup LO-ownership correction, the
> `MD→PDF` LO-vs-park gate, the `RTF→markup` pandoc-vs-LO `[DEFER: corpus]`),
> [`04-formats/spreadsheets`](../spec/04-formats/spreadsheets.md) (workbook pairs,
> the native CSV↔TSV carve-out, multi-sheet picker, encoding/delimiter/formula
> policy, CSV-injection-safe import), [`04-formats/presentations`](../spec/04-formats/presentations.md)
> (slide pairs, the asymmetric MS-family loss, ExportNotesPages),
> [`03-engines-and-bundling`](../spec/03-engines-and-bundling.md) (§3.5.2 LibreOffice
> disposable-profile + macro/link/Calc-external hardening + output discovery +
> exit-0-but-wrote-nothing rule, §3.5.3 poppler, §3.5.4 pandoc `--sandbox`, §3.5.6
> native CSV/TSV, §3.6/§3.7 copyleft isolation + SBOM/NOTICE, §3.9.3 bundled fonts),
> [`06-build-test-release §6.5`](../spec/06-build-test-release.md) (the reliability
> gate + the two permissible exceptions + engine-bump re-validation). Index:
> [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **Reads, never re-decides:** the P4-built **generic** machinery — the §2.12
> isolation wrapper (`crate::isolation`), the §1.7 per-item lifecycle, the §6.4.3
> per-pair runner, the §6.5.2 pair-status ledger generator, the §6.4.3a corpus↔pair
> bijection guard, the §1.6 options-panel shell + lossy-note + progress/cancel +
> result-actions chrome, the §3.4 patent matrix + `EngineHealth` wiring (no office
> format carries a patent flag — every office pair always resolves), the generic
> §6.1.3 build-assertion framework, the SBOM/NOTICE scaffold, the §7.2.3 startup
> verifier, the §3.5.0/§7.2.6 macOS TCC source-staging copy, and the `scripts/
> stage-engines` skeleton + the pinned checksum-verified engine-asset cache. The
> §1.2 layered-detection dispatcher + the text/encoding/delimiter sniff + the native
> CSV↔TSV transform + RFC-4180 re-quote + the CSV-injection literal-preservation
> rule are **P3's** — P7 adds only the per-format office signatures and the LO-owned
> CSV/TSV workbook pairs, not the native text pass.
>
> **This is the v0 BASE** — the smallest-atomic `[ ]` boxes are below, grouped under
> `### ` sub-headings; a later adversarial review will deepen, split, reconcile (incl.
> P0's `→ activated in P7` / per-engine `→ executed in P7` cross-refs against these
> real box-ids) and complete them. Pairs are grouped by **engine code-path** (one
> filter / one export path / one sniff = one group) because a code-path is the
> smallest unit genuinely built once and then exercised across the pairs that share
> it; each pair still gets its own corpus backing, integration test and ledger row,
> so no pair "hides" inside a group.

## Boundaries (read against P3 + P4)

- **P3 ↔ P7:** P3 built the **native CSV/TSV engine** (streamed pass, encoding/
  delimiter sniff, RFC-4180 re-quote, CSV-injection literal-preservation) and the
  §1.2 detection-dispatcher skeleton with the CSV/TSV text path + first KAT entries.
  **P7 broadens** the text path with the office container signatures (OOXML/ODF/OLE2
  disambiguation) and wires the LO-owned CSV/TSV *workbook* pairs — it must **not**
  re-implement the native transform, the text sniff, or the detection dispatcher.
- **P4 ↔ P7:** P4 built the **generic** harness (isolation wrapper, lifecycle, the
  per-pair runner + ledger + bijection guard, options-panel shell, SBOM/NOTICE
  scaffold, §6.1.3 framework, startup verifier, macOS TCC staging, the engine-asset
  cache + `stage-engines` skeleton). **P7 fills the office-specific variants:** the
  three engines' staging + hardening + `engines.lock`/SBOM rows + §6.1.3 assertion
  lists + §7.2.3 availability rows, every office pair, the corpus, the per-pair
  tests, the option **declarations** (chrome already built), the bundled-font
  staging, and the per-engine SSRF/LFR hardening (LO profile, pandoc `--sandbox`,
  poppler no-network). P7 must **not** rebuild the panel chrome, the runner, the
  ledger generator, the isolation wrapper, or the patent matrix.

## Internal §6.5 sub-gates (intra-phase milestones)

LibreOffice is the shared workhorse of all three categories and the size driver of
the whole product, so it is staged + hardened **first** and a §6.5 sub-gate marks
**every spreadsheet + presentation pair `reliable`** (the LO-only categories) before
the document category — whose pandoc/poppler engines and the `MD→PDF` ship-or-park
gate add the remaining risk — is attempted. The sub-gate boxes sit between the
clusters and `needs:` their cluster's pair boxes.

---

### LibreOffice engine staging, hardening & runtime wiring

> The LibreOffice headless sidecar (program tree + disposable-profile template +
> bundled fonts) must exist, be hardened (the disposable `-env:UserInstallation`
> profile, the `registrymodifications.xcu` macro/link/Calc-external pins),
> checksum-verified, SBOM/NOTICE-rowed and runtime-wired through the §2.12 boundary
> before any office pair can be built. These boxes execute the per-engine variants
> of the P0.7-policy / P4-framework gates for LibreOffice specifically.

- [ ] **P7.1** [BUILD] Stage the LibreOffice headless program tree as a `bundle.resources` dir per-OS (cache-keyed) · §3.3.1 §3.3.2 §6.1.3 · G37
  needs: P4.27, P6.93
  > `scripts/stage-engines` restores the `actions/cache`-hosted `libreoffice-<ver>-<triple>` engine-asset cache (checksum-verified pinned-URL fetch on a miss) and places the LibreOffice **directory tree** (`program/soffice.bin` launcher + `program/`, `share/`, type libraries) under `src-tauri/resources/libreoffice/` as a `bundle.resources` map (NOT `externalBin` — it is a tree, not a single self-contained exe, §3.3.1). The one MPL-2.0 binary serves documents/spreadsheets/presentations. → executes the P0.7.3/P0.7.4 acquisition+staging policy for LibreOffice.
- [ ] **P7.2** [BUILD] Stage the §3.9.3 bundled-font baseline beside the LibreOffice sidecar · §3.9.3 §6.1.3 · G37 G35 G36
  needs: P7.1
  > stage the `[DECIDED]` baseline font set under `src-tauri/resources/fonts/` as `bundle.resources` — Liberation + Carlito + Caladea (metric-compatible Arial/Calibri/Cambria/Times/Courier) + the curated Noto subset (Sans/Serif CJK-SC/TC/JP/KR Regular + Noto Sans Arabic/Hebrew) so LibreOffice substitution is graceful and non-Latin text never tofus; the single biggest fidelity lever for all three categories; each font is a first-class `engines.lock`/SBOM row (SHA-256 + SPDX OFL-1.1/Apache-2.0 + source URL — the Liberation OFL-1.1 trap, Carlito/Caladea Apache-2.0, Noto CJK OFL-1.1). CJK weight breadth is the `[DEFER: size]` knob, not a design call.
- [ ] **P7.3** [BUILD] Anchor the LibreOffice acquisition + add its `engines.lock` row + SBOM/NOTICE rows · §3.7.2 §3.8 · G37 G35 G36
  needs: P7.1
  > add the LibreOffice `engines.lock` row (`purl` `pkg:generic/libreoffice@<ver>` + SHA-256 + a CPE where one exists) per the P0.7.3 acquisition policy (from-source signed-tarball OR ≥2-mirror/distro-signed prebuilt corroboration); populate the CycloneDX SBOM + `THIRD-PARTY-LICENSES` rows for LibreOffice MPL-2.0 + its many bundled components (Syft cross-check); the bundled security-CONFIG file `registrymodifications.xcu` (P7.5) is itself a first-class `engines.lock` row (SHA-256 + SPDX + source). → executes the P0.7.1/P0.7.3 policy for LibreOffice.
- [ ] **P7.4** [RUST] Wire the LibreOffice invocation through the §2.12 isolation boundary with the minimal-env + loader-strip + cwd=scratch contract · §3.5.2 §2.12 §2.14 · G29
  needs: P7.1, P4.13
  > register LibreOffice in the §3.2 `Engine` registry (`EngineProgram::ResourceBin`, resolved via `app.path().resolve("engines/libreoffice/program/soffice", BaseDirectory::Resource)` — NOT externalBin); route every invocation through the P4 §2.12 isolation wrapper with cwd = per-run scratch (§2.14), minimal isolated env (`LC_ALL=C.UTF-8`, no proxy vars, `PATH` not relied on — absolute bundled path), and the explicit dynamic-loader-injection strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_INSERT_LIBRARIES`/`DYLD_LIBRARY_PATH` cleared, G29 `.env_clear()` invariant). Untrusted office files (zip-bomb OPC, malformed OOXML, macro-bearing) parsed in LO = classic T1 attack surface.
- [ ] **P7.5** [BUILD] Author + seed the disposable `-env:UserInstallation` profile + the hardened `registrymodifications.xcu` (macros + links) · §3.5.2 §0.11 · G38 G37
  needs: P7.1
  > the T1-macro-RCE control: pre-seed the disposable per-run profile template with a `registrymodifications.xcu` pinning **macro security at the highest level** (`…/Security/Scripting/MacroSecurityLevel = 3` disable-all-without-notification + `DisableMacrosExecution = true`, no Basic IDE — macros never run, no prompt blocks headless) and **no link auto-update on load** (`LinkUpdateMode = 0` so external-reference/DDE/remote-OLE links don't fetch on load) and **no remote/OLE auto-fetch**. The §6.1.3 build assertion checks the staged `.xcu` carries these keys (P7.7). The concrete mechanism behind every category's "macros never executed / dropped" policy.
- [ ] **P7.6** [BUILD] Pin the Calc external-data / WEBSERVICE / external-reference T9b profile keys (best-effort, defence-in-depth) · §3.5.2 §0.11 · G38
  needs: P7.5
  > extend the `registrymodifications.xcu` with the Calc T9b external-data vectors (best-effort): **no external-data-range refresh on load**, **no external-reference recalculation on load** (`…/Office/Calc/.../Load` external-reference update off), **linked-object/DDE auto-update off** (composing with `LinkUpdateMode = 0`). These are **defence-in-depth, not the load-bearing proof** — the office-engine T9b half leans on the §2.11.4 packet-monitor gate + the §6.4.2 adversarial-egress Calc case (a crafted `.xlsx` with a `WEBSERVICE`/external-data trigger → zero egress AND no out-of-input read) for its release-blocking proof, exactly as FFmpeg/pandoc are corpus-proven.
- [ ] **P7.7** [BUILD] Add the LibreOffice §6.1.3 build assertions — profile keys present + filter availability + no-network · §6.1.3 §3.5.2 · G38
  needs: P7.5, P4.51
  > the per-engine §6.1.3 assertion list (fills the P4 generic framework): parse the staged `registrymodifications.xcu` and FAIL the build if the macro-disable / `LinkUpdateMode=0` / Calc-external keys (P7.5/P7.6) are absent; assert the staged LO build is **not** wired for network link-fetch; assert every filter name ConvertIA invokes (P7.8) exists in the staged build (the exposed-parameter capability assertion). → executes the P0.7.4 per-engine build-assertion policy for LibreOffice.
- [ ] **P7.8** [RUST] Wire the LibreOffice invocation SHELL — argv assembly, `--outdir`, headless/nolockcheck/disposable-profile flags · §3.5.2 · G31
  needs: P7.4
  > the §3.5.2 argv SHELL (pure Rust dispatch — no filter-name data): `soffice --headless --norestore --nolockcheck --nodefault --nofirststartwizard -env:UserInstallation=file://<per-run-profile> --convert-to <ext>:<FilterName>[:<FilterData-JSON>] --outdir <unique-empty-scratch> <input>` — the command-line / arg-vector construction, the `--outdir` slot, the flag set, and the `<FilterName>`/`<FilterData-JSON>` token *positions* (the values come from P7.9/P7.14). P7.10–P7.16 build on this invocation shape. Split from the filter-name table (P7.9): an invocation-shape error fails a different check than a filter-table error, and P7.10–P7.16 need only the invocation shape while P7.14 (FilterData assembly) also needs the filter names.
- [ ] **P7.9** [RUST] Wire the fixed LibreOffice filter-name table for all LO-owned categories (the data-driven registry) · §3.5.2 · G31
  needs: P7.4
  > the fixed filter-name registry (a data-driven table with its own serialization/validation, distinct from the P7.8 invocation shell) — `*→PDF`: `writer_pdf_Export`/`calc_pdf_Export`/`impress_pdf_Export`; office↔office: `MS Word 2007 XML`/`MS Word 97`/`writer8`/`Rich Text Format`, `Calc MS Excel 2007 XML`/`MS Excel 97`/`calc8`, `Impress MS PowerPoint 2007 XML`/`MS PowerPoint 97`/`impress8`; CSV/TSV: `Text - txt - csv (StarCalc)` + FilterOptions token string; DOC→markup: `Text`/`HTML (StarWriter)`/`Markdown`†. The per-pair filter VALUES are owned by the §04 files; this box owns the canonical name→filter map the P7.8 invocation interpolates and the P7.14 FilterData assembly references.
- [ ] **P7.10** [RUST] Wire the one-document-per-invocation + per-run disposable-profile + §0.9 LibreOffice serialization · §3.5.2 §0.9 §2.6 · G31
  needs: P7.8
  > each LO job gets its **own** disposable `-env:UserInstallation` profile in per-run scratch (§2.14), torn down with the run (§2.6); the §0.9 pool reads `descriptor().serialised_only = true` for LibreOffice and acquires the single-permit semaphore BEFORE spawn (parallel instances on one profile lock/corrupt) — one document per invocation, serialized. The §3.2.2 `EngineDescriptor.serialised_only` data path the pool depends on is populated here for LO.
- [ ] **P7.11** [RUST] Wire the LibreOffice output discovery via unique-empty-outdir snapshot-diff (never source-basename match) · §3.5.2 §2.1 · G31
  needs: P7.8
  > the §3.5.2 `[DECIDED]` discovery rule: LO normalises/truncates basenames, so the core must NOT string-match the source basename. Each job gets a **unique, empty, per-job `--outdir`** under per-run scratch; discovery = snapshot-diff (list outdir empty before spawn, list after verified success, pick the single new `*.<ext>` file); that discovered file is atomic-published to the planned final name (§2.1) — LO's own output naming is never the user-facing name.
- [ ] **P7.12** [RUST] Wire the LibreOffice exit-0-but-wrote-nothing success verification + the password-protected → §2.8 mapping · §3.5.2 §2.8 · G31
  needs: P7.11
  > the critical correctness rule: LO headless returns **exit 0 even on some failures** and writes nothing → success is verified by the expected output file **existing and being non-empty** in `--outdir`, NOT by exit code. **Zero** new files despite exit 0 → mapped per the stderr rule; encrypted/password files → no output → the §2.8 "password-protected" kind (shared by documents/spreadsheets/presentations); a stale soffice lock avoided by the per-run profile + `--nolockcheck`.
- [ ] **P7.13** [RUST] Wire the LibreOffice exit/stderr → §2.8 error-kind mapping (`classify_failure`) + cancellation + no-partial-output · §3.5.2 §2.8 §1.7 §2.1 · G31
  needs: P7.12
  > map LO stderr/no-output patterns to §2.8 kinds (password-protected; corrupt/partial OPC zip or CFB → corrupt; generic → plain-language engine-failure §2.13); cancellation via §1.7 process-group kill (a cancelled LO job leaves NO partial output — LO writes into the scratch outdir, atomic-published only on verified non-empty success); a crashing/hanging LO fails THAT one item and the batch continues (§1.9).
- [ ] **P7.14** [RUST] Wire the FilterData-JSON PDF-export option assembly (shared by all `*→PDF`) · §3.5.2 §1.6 · G31
  needs: P7.8, P7.9
  > assemble the inline FilterData JSON for the PDF export filters — e.g. `pdf:impress_pdf_Export:{"ExportNotesPages":{"type":"boolean","value":"true"}}`; the typed→JSON wire form for `SelectPdfVersion`/`UseTaggedPDF`/`Quality`/`ExportBookmarks`/`ReduceImageResolution`/`ExportNotesPages`/`MaxImageResolution`/`UseLosslessCompression`/`EmbedStandardFonts`; the per-category default VALUES are owned by the §04 files (P7.25/P7.44) — this box owns only the assembly, with a `PlanError` (range) for an out-of-range value.
- [ ] **P7.15** [TEST] Add the per-engine LibreOffice §7.2.3 availability/integrity rows + the in-bundle hash-manifest entries · §7.2.3 · G46 G37
  needs: P7.3, P4.43
  > populate the LibreOffice launcher + program-tree + bundled-font rows in the build-time in-bundle hash manifest and the `EngineHealth` availability table (the per-engine variant of the P4 startup-verifier framework) so a missing/corrupt LibreOffice escalates to a §2.13 app-fault not a crash, and feeds C12 `get_engine_health` (§5.2 disables the office targets if LO is unavailable — all three categories depend on it).
- [ ] **P7.16** [RUST] Verify LibreOffice receives the macOS kind-2 scratch-staged source + the staged `--outdir` rule · §3.5.0 §7.2.6 §2.14.2 · G31 G29
  needs: P7.4, P4.24
  > assert (macOS only) the core stages the dropped source into per-job kind-2 scratch BEFORE spawning LO and hands LO the SCRATCH path as `<input>` (with `--outdir` already at scratch), so a spawned engine is never the first process to touch a TCC-protected Desktop/Documents/Downloads/removable path (T11); composes with §2.14 cross-volume + the §1.10 macOS staged-input preflight. The `stage_for_tcc`-before-spawn invariant (G29 Semgrep rule) holds for the LO `Command::new`.

---

### poppler `pdftotext` engine staging, hardening & wiring (PDF→TXT)

> The single PDF-consuming engine. Separate invoked GPL binary; no Ghostscript
> backstop (`[DECIDED: dropped v1]`); poppler-only with a clean fail-clearly on the
> unrecoverable minority. One pair: `PDF→TXT`.

- [ ] **P7.17** [BUILD] Stage the poppler `pdftotext` sidecar per-OS + add its `engines.lock`/SBOM/NOTICE rows · §3.3 §3.7.2 §3.8 · G37 G35 G36
  needs: P4.27, P0.7.3
  > `scripts/stage-engines` restores the `poppler-<ver>-<triple>` cache (checksum-verified pinned-URL on miss; the from-source built-without-network populate path is P7.17.1), places `pdftotext` under `src-tauri/binaries/` target-triple-suffixed, declares it in `tauri.conf.json` `bundle.externalBin`; add the `engines.lock` row (`purl` `pkg:generic/poppler@<ver>` + SHA-256 + poppler CPE) per the P0.7.3 policy; SBOM/`THIRD-PARTY-LICENSES` rows with the **valid SPDX expression `GPL-2.0-only OR GPL-3.0-only`** (not the bare `GPL-2.0/GPL-3.0` — §6.3.3 rejects unresolved) + the GPL written-offer-of-source corresponding-source pointer line. → executes P0.7.1/P0.7.3 for poppler (`needs: P0.7.3` for the from-source acquisition + engine-source-allow-list policy this anchors against; the cross-phase edge carried via the P7.77 reconciliation box).
  - [ ] **P7.17.1** [BUILD] Compile poppler `pdftotext` from source as the built-without-network / no-Ghostscript build via the P4.28.1 harness (fills the poppler configure-flag manifest seam) · §6.1.3 §3.5.3 §3.1 · G37
    needs: P4.28.1
    > the from-source poppler build the **P7.18 built-without-network + no-Ghostscript §6.1.3 assertions can only pass against** where a stock prebuilt enables a network fetch path or pulls a Ghostscript backstop (AGPL surface): compile poppler/`pdftotext` through the **P4.28.1 from-source compilation harness** with the curated configure flags (no network/remote-URI fetch path so a crafted PDF referencing a remote resource produces no egress — the §3.4.3 remote-URI sentinel; no Ghostscript dependency, the §3.1/§3.6 AGPL-free posture), filling the P4.28.1 per-engine `poppler.configure.flags` manifest seam so the configure line is the data P7.18 cross-checks; populate the `poppler-<ver>-<triple>` cache key P7.17 staging reads. (Where a distro-signed prebuilt provably ships built-without-network + GS-free, the P0.7.3 prebuilt corroboration branch may anchor it instead — but the assertion still tests the curated property, so the from-source compile is the satisfiable path when no such prebuilt exists.) (`needs: P4.28.1` for the from-source harness; the cross-phase edge carried via the P7.77 reconciliation box.)
- [ ] **P7.18** [BUILD] Assert the poppler built-without-network + no-Ghostscript-backstop §6.1.3 build assertions · §6.1.3 §3.5.3 §3.1 · G38
  needs: P7.17, P4.51, P0.7.4
  > the per-engine §6.1.3 assertions: assert the staged poppler/`pdftotext` is built without a network fetch path (the §3.4.3 remote-URI sentinel — a crafted PDF referencing a remote resource produces no egress) and that **no Ghostscript binary is staged** (GS `[DECIDED: NOT shipped v1]`, AGPL surface removed); record the absence as the §3.1/§3.6 AGPL-free posture. → executes the P0.7.4 per-engine build-assertion policy for poppler (`needs: P0.7.4`, the assertion-policy home, `[x]` before the loop; the cross-phase edge carried via the P7.77 reconciliation box).
- [ ] **P7.19** [RUST] Wire `pdftotext` through the §2.12 boundary + the fixed argv (`-enc UTF-8 -eol unix`, no `-layout`) · §3.5.3 §2.12 §2.14 · G29 G31
  needs: P7.17, P4.13
  > register poppler in the §3.2 registry (`EngineProgram::Sidecar`, resolved bare-name beside the app exe); route through the §2.12 isolation wrapper (cwd=scratch, minimal env, loader-strip, G29 `.env_clear()`); fixed argv `pdftotext -enc UTF-8 -eol unix <input> <out_tmp.txt>` — `-layout` NOT used by default (plain reading order is the everyday "get the words out"; documents.md owns the lossy note `doc_pdf_to_text`). `CoarseSpawnDone` progress.
- [ ] **P7.20** [RUST] Wire the poppler exit/stderr → §2.8 mapping (encrypted/empty-extraction/unrecoverable) + `classify_failure` · §3.5.3 §2.8 · G31
  needs: P7.19
  > map `pdftotext` outcomes: non-zero / "Command Line Error: Incorrect password" on an encrypted no-user-password PDF → the §2.8 "password-protected" kind (no password ever prompted/cracked); **empty extraction** (scanned/image PDF, no OCR in v1) → a valid-but-near-empty `.txt` reported honestly, **not** an error and **not** a misleading success of an empty file; an unrecoverable PDF → fail clearly (§2.8), batch continues (no GS repair backstop in v1). PDF forms/tagged structure/layers flattened to visible text on `→TXT`.
- [ ] **P7.21** [RUST] Verify poppler receives the macOS kind-2 scratch-staged source path · §3.5.0 §7.2.6 §2.14.2 · G31 G29
  needs: P7.19, P4.24
  > assert (macOS only) the core stages the dropped PDF into per-job kind-2 scratch before spawning `pdftotext` and hands it the scratch path as `<input>` (T11 — engine never the first to touch a protected path); the `stage_for_tcc`-before-spawn G29 invariant holds for the poppler `Command::new`.
- [ ] **P7.22** [TEST] Add the poppler §7.2.3 availability/integrity row + the in-bundle hash-manifest entry · §7.2.3 · G46 G37
  needs: P7.17, P4.43
  > populate the `pdftotext` row in the in-bundle hash manifest + the `EngineHealth` availability table so a missing/corrupt poppler degrades the `PDF→TXT` target to unavailable-with-reason (§5.2) rather than crashing, and feeds C12 `get_engine_health`.

---

### pandoc engine staging, hardening & wiring (markup conversions)

> The markup engine for the XML/text sources (`DOCX/ODT/RTF/MD/HTML/TXT ↔`). Always
> `--sandbox` (the cheap-tier SSRF/LFR control). pandoc **cannot** read legacy
> binary `.doc` (those down-conversions are LO-owned, P7.40); the registry never
> hands pandoc a `.doc`. The `RTF→markup` ownership is a `[DEFER: corpus]` (pandoc
> default, LO fallback) resolved by P7.63.

- [ ] **P7.23** [BUILD] Stage the pandoc sidecar per-OS + add its `engines.lock`/SBOM/NOTICE rows + the `--version ≥ 2.15` floor · §3.3 §3.7.2 §3.8 §6.1.3 · G37 G35 G36 G38
  needs: P4.27
  > `scripts/stage-engines` restores the `pandoc-<ver>-<triple>` cache (checksum-verified pinned-URL on miss), places `pandoc` under `src-tauri/binaries/` target-triple-suffixed, declares it in `bundle.externalBin`; add the `engines.lock` row (`purl` `pkg:generic/pandoc@<ver>` + SHA-256 + pandoc CPE) + SBOM/`THIRD-PARTY-LICENSES` rows (GPL-2.0+ + written-offer corresponding-source pointer); the §6.1.3 build assertion asserts the staged pandoc reports **`--version ≥ 2.15`** (the `--sandbox` floor) and FAILS the build below it. → executes P0.7.1/P0.7.3/P0.7.4 for pandoc.
- [ ] **P7.24** [RUST] Wire pandoc through the §2.12 boundary + the always-on `--sandbox` SSRF/LFR control · §3.5.4 §3.3.4 §2.12 §0.11 · G29 G42 G42b
  needs: P7.23, P4.13
  > register pandoc in the §3.2 registry (`EngineProgram::Sidecar`); route through the §2.12 isolation wrapper (cwd=scratch, minimal env, loader-strip, G29 `.env_clear()`); **every pandoc invocation runs with `--sandbox`** (≥2.15) — confines readers/writers to the named file(s) and blocks all network + file-system reads from the document (a crafted MD/HTML/RST/Org/LaTeX include or remote `<img>`/CSS cannot pull a remote/local out-of-input file). This is the load-bearing markup-engine SSRF/LFR control (the §3.3.4 "pandoc fetches nothing" claim) — corpus-proven by the §6.4.2 adversarial-egress case, not the registry. No pandoc Lua/JSON filters and no pandoc PDF production configured (so the documented `--sandbox` gaps don't apply).
- [ ] **P7.25** [RUST] Wire the fixed pandoc option set (`--wrap=preserve`, `-f gfm`/`-t gfm`, `*→HTML --standalone --embed-resources`) + stdin plan · §3.5.4 · G31
  needs: P7.24
  > the §3.5.4 concrete opts: `pandoc -f <in-fmt> -t <out-fmt> [opts] -o <out_tmp> <input>` (or stdin via `StdinPlan::PipeBytes` for awkward paths); `--wrap=preserve` always; `*→HTML` adds `--standalone --embed-resources` (self-contained single file); MD read dialect `-f gfm`; `*→MD` writes `-t gfm`. The per-pair `-f`/`-t` format codes are owned by the §04 pairs (P7.31–P7.39).
- [ ] **P7.26** [RUST] Wire the pandoc exit/stderr → §2.8 mapping (`classify_failure`) + cancellation + no-partial-output · §3.5.4 §2.8 §1.7 §2.1 · G31
  needs: P7.24
  > map pandoc non-zero + message → §2.8 generic plain-language engine-failure (the "openBinaryFile … does not exist" case never occurs — the core verifies the input before spawn); cancellation via §1.7 process-group kill; a cancelled pandoc job leaves NO partial output (writes into the §2.1 `out_tmp`, atomic-published only on success); `CoarseSpawnDone` progress.
- [ ] **P7.27** [TEST] Verify pandoc runs cleanly under `--sandbox` for every assigned pair (no blocked on-disk data file) · §3.5.4 §6.4 · G31
  needs: P7.24
  > the `[DEFER: corpus]` data-file check: confirm every pair ConvertIA assigns pandoc (markup↔markup, `*→HTML --standalone --embed-resources`, the office→markup down-conversions) runs cleanly under `--sandbox` on the §6.4 corpus — none needs a blocked on-disk pandoc data file (templates, reference docs, syntax-highlight definitions). If a pair turns out to need one, the recorded fix is to **bundle that data file and pass it explicitly on the argv** (a named input the sandbox permits), NEVER to drop `--sandbox`. Records the resolution against real corpus files.
- [ ] **P7.28** [RUST] Verify pandoc receives the macOS kind-2 scratch-staged source (path or stdin) · §3.5.0 §7.2.6 §2.14.2 · G31 G29
  needs: P7.24, P4.24
  > assert (macOS only) the core stages the source into per-job kind-2 scratch before spawning pandoc and feeds it the scratch path as `<input>` OR pipes bytes on stdin (`StdinPlan::PipeBytes`) — engine never the first to touch a protected path (T11); the `stage_for_tcc`-before-spawn G29 invariant holds for the pandoc `Command::new`.
- [ ] **P7.29** [TEST] Add the pandoc §7.2.3 availability/integrity row + the in-bundle hash-manifest entry · §7.2.3 · G46 G37
  needs: P7.23, P4.43
  > populate the `pandoc` row in the in-bundle hash manifest + the `EngineHealth` table so a missing/corrupt pandoc degrades the pandoc-owned markup targets to unavailable-with-reason rather than crashing, and feeds C12 `get_engine_health`.

---

### Office-format detection signatures (the §1.2 container-disambiguation broadening)

> The headline detection risk for the whole phase: OOXML/ODF/`.epub` all share the
> ZIP magic, and the OLE2 (CFB) magic is shared by legacy `.doc`/`.xls`/`.ppt`.
> Detection MUST look inside (content over name, §1.2 / SSOT Principle 6), never
> trust the extension. These boxes add the office signatures to the P3-built §1.2
> dispatcher (which already owns the text/CSV/TSV path) — the activation target for
> the P0.5.7 KAT convention + the P0.4.3 detect-fuzz target.

- [ ] **P7.30** [RUST] Wire the ZIP/OPC container content-type disambiguation (DOCX vs XLSX vs PPTX vs ODF) · §1.2 · G15 G31
  needs: P3.26
  > extend the §1.2 dispatcher: a `50 4B 03 04` (`PK`) leader peeks inside the OPC archive's `[Content_Types].xml` — WordprocessingML + `word/document.xml` ⇒ **DOCX**; `…spreadsheetml…` + `xl/workbook.xml` ⇒ **XLSX**; `…presentationml.*` + `ppt/presentation.xml` ⇒ **PPTX**; the uncompressed first-stored `mimetype` member ⇒ ODF (`…opendocument.text`⇒**ODT**, `…spreadsheet`⇒**ODS**, `…presentation`⇒**ODP**). A `.docx` that is really an XLSX/ODS/PPTX is classified by its inner manifest, never its name. The container parse stays bounded/memory-safe (no third-party C/C++ decoder pre-detect, §2.12.4) and feeds the decompression-bomb-in-OPC bound (P3 zip-slip/ratio caps). **KAT-entry deliverable (same commit, G15):** for EACH format disambiguated here, add a `tests/detect-kat.toml` entry pinning its canonical fixture to its exact `FormatId` (+ the genuinely-ambiguous **DOCX-vs-XLSX-vs-PPTX-vs-ODF-vs-bare-ZIP** shared-`PK` cases — incl. a plain `.zip` that is NOT an OPC document, classified as ZIP/unsupported, never mis-promoted to DOCX) so a mis-wired container-disambiguation (a DOCX classified as bare ZIP, or vice versa) is caught at L2 by the G15 KAT test, not only at the per-pair L4 corpus run; the per-format-≥1-KAT-entry completeness is asserted by the P4.60.5 gate.
- [ ] **P7.31** [RUST] Wire the OLE2/CFB stream-directory disambiguation + the text-magic signatures (two independent §1.2 parser surfaces) · §1.2 · G15 G31
  needs: P3.26
  > extend the §1.2 dispatcher with two **independently-writable, independently-testable** detection surfaces (different fixture sets) — split into the two sub-boxes so a failure is attributable. The container parse stays bounded/memory-safe (no third-party C/C++ decoder pre-detect, §2.12.4).
  - [ ] **P7.31.1** [RUST] Wire the OLE2/CFB compound-file CLSID/stream-directory disambiguation (DOC vs XLS vs PPT) · §1.2 · G15 G31
    > a `D0 CF 11 E0 A1 B1 1A E1` (CFB) leader reads the internal stream directory — `WordDocument` ⇒ **DOC**, `Workbook`/`Book` ⇒ **XLS**, `PowerPoint Document` ⇒ **PPT** (the shared-OLE2 disambiguation, the headline collision); the `.docm`/`.xlsm`/`.pptm`/`.ppsx`/`.pps`/`.otp`/`.potx` macro/template/autoplay variants mapped to their base class, keyed onto the resulting `UserFacingFormat` (§1.3 grouping). **KAT-entry deliverable (same commit, G15):** add a `tests/detect-kat.toml` entry pinning DOC/XLS/PPT canonical fixtures to their exact `FormatId` plus the genuinely-ambiguous **DOC-vs-XLS-vs-PPT shared-OLE2** stream-directory disambiguation case (the headline collision) so a mis-wired OLE2 disambiguation is caught at L2 by the G15 KAT test, not only at L4; the per-format-≥1-KAT-entry completeness is asserted by the P4.60.5 gate.
  - [ ] **P7.31.2** [RUST] Wire the text-magic signatures (RTF / HTML / PDF) + the flat-XML `.fods` + the MD-vs-TXT intent rule · §1.2 · G15 G31
    > RTF `7B 5C 72 74 66` (`{\rtf`) at offset 0; PDF `25 50 44 46 2D` (`%PDF-`) tolerating a short junk prefix; HTML sniff (`<!DOCTYPE html`/`<html`/leading `<` HTML-ish, case-insensitive, BOM/whitespace-tolerant); the `.fods` flat-XML ⇒ ODS-family; MD vs TXT is by **extension/intent** (`.md`⇒MD, `.txt`⇒TXT — Markdown is valid plain text), keyed onto the resulting `UserFacingFormat` (§1.3 grouping). **KAT-entry deliverable (same commit, G15):** add a `tests/detect-kat.toml` entry pinning RTF/HTML/PDF/.fods/MD/TXT canonical fixtures to their exact `FormatId` (+ the MD-vs-TXT intent case and the HTML-vs-leading-`<`-XML edge) so a mis-wired text-magic classification is caught at L2 by the G15 KAT test, not only at L4; the per-format-≥1-KAT-entry completeness is asserted by the P4.60.5 gate.

---

### Document category: pandoc markup down-conversions (XML/text sources → TXT/MD/HTML)

> `DOCX/ODT/RTF → TXT/MD/HTML` via pandoc (reads them natively). All `✓~` lossy
> (formatting/layout simplified). DOC→markup is LO-owned (P7.40 — pandoc can't read
> binary `.doc`). Each pair box `needs:` the shared pandoc runtime wiring.

- [ ] **P7.32** [RUST] Wire `DOCX → TXT/MD/HTML` (pandoc, `-f docx`) · §3.5.4 · G31 G32
  needs: P7.25
  > register `DOCX→TXT` (`-t plain`), `DOCX→MD` (`-t gfm`), `DOCX→HTML` (`-t html --standalone --embed-resources`) via pandoc; `--wrap=preserve`; lossy `doc_to_text` (TXT) / `doc_simplified` (MD) / `doc_simplified` (HTML); embedded images extracted/inlined into HTML, dropped-with-note for TXT and bare MD (the `[DEFER: corpus]` `*→MD` image policy leans **drop-with-note**, validated P7.62).
- [ ] **P7.33** [RUST] Wire `ODT → TXT/MD/HTML` (pandoc, `-f odt`) · §3.5.4 · G31 G32
  needs: P7.25
  > register `ODT→TXT`/`ODT→MD`/`ODT→HTML` via pandoc (reads ODT natively); same opt set + lossy kinds as DOCX; ODT is LibreOffice's home format but the markup down-conversions stay pandoc (cleaner/lighter HTML/MD per the documents.md single-owner resolution).
- [ ] **P7.34** [RUST] Wire `RTF → TXT/MD/HTML` (pandoc default, LO `[DEFER: corpus]` fallback) · §3.5.4 · G31 G32
  needs: P7.25
  > register `RTF→TXT`/`RTF→MD`/`RTF→HTML` via pandoc's RTF reader as the v1 default; same opt set + lossy kinds; **ownership is `[DEFER: corpus]`** — pandoc's RTF reader has known gaps (super/subscript, complex tables) and if the corpus shows it too lossy, ownership falls back to LibreOffice's markup export (P7.63 resolves; the registry stays single-owner whichever way it resolves). RTF code-page header drives encoding so non-Latin text survives.
- [ ] **P7.35** [RUST] Wire `HTML → TXT/MD` (pandoc, `-f html`) · §3.5.4 · G31 G32
  needs: P7.25
  > register `HTML→TXT` (tags stripped → plain text, `doc_to_text`) and `HTML→MD` (rich HTML simplified to Markdown, `doc_simplified`) via pandoc; **single-file HTML only** in v1; JavaScript never executed; external CSS/images by remote URL not fetched (offline + `--sandbox`); `<meta charset>`/BOM honored.

---

### Document category: pandoc markup up-conversions (TXT/MD/HTML → office/markup)

> The "bring lightweight text into a richer document" direction via pandoc — all
> faithful (`—`, not lossy) except `MD→TXT` (strips syntax). `*→DOC` is NOT offered
> from TXT/MD/HTML (no everyday `markdown→.doc` demand — matrix `—`). `*→PDF` from
> these sources is LO-owned (P7.41), NOT pandoc (no chained pandoc→LaTeX step).

- [ ] **P7.36** [RUST] Wire `TXT → DOCX/ODT/RTF/MD/HTML` (pandoc) + the UTF-8-no-BOM output rule · §3.5.4 §2.10 · G31 G32
  needs: P7.25
  > register `TXT→DOCX`/`TXT→ODT`/`TXT→RTF`/`TXT→MD`/`TXT→HTML` via pandoc (input read as plain/markdown, target written); **not lossy** (plain text has nothing to lose — only the reverse `*→TXT` is); output encoding fixed to **UTF-8 (no BOM default)** — the content-fidelity guarantee (§2.10); CR/LF normalized on the target's terms; mixed-encoding/invalid bytes → fail clearly rather than emit mojibake. NO "output encoding" toggle (`[DECIDED]` out of v1).
- [ ] **P7.37** [RUST] Wire `MD → HTML/DOCX/ODT/RTF/TXT` (pandoc) + the gfm dialect + local-only image resolution · §3.5.4 · G31 G32
  needs: P7.25
  > register `MD→HTML`/`MD→DOCX`/`MD→ODT`/`MD→RTF`/`MD→TXT` via pandoc (input `-f gfm`); `MD→HTML` adds `--standalone --embed-resources`; all faithful **except `MD→TXT`** (strips syntax → plain prose, `doc_to_text`). Local relative image refs resolved/embedded where `--sandbox` allows; **remote URLs NOT fetched** (offline) → broken refs, noted; raw HTML passed through; fenced code monospaced; YAML front-matter parsed as metadata not printed. (NB: `MD→PDF` is LO-only, P7.42 — pandoc has NO chain-free PDF path here.)
- [ ] **P7.38** [RUST] Wire `HTML → DOCX/ODT/RTF` (pandoc) · §3.5.4 · G31 G32
  needs: P7.25
  > register `HTML→DOCX`/`HTML→ODT` (faithful, `—`) and `HTML→RTF` (`✓`, rich features simplified) via pandoc; single-file HTML only; JS never executed, remote assets not fetched; relative local assets resolved where the sandbox allows.
- [ ] **P7.39** [TEST] Assert the `*→DOC`-from-markup absent-target rule (TXT/MD/HTML→DOC is `—`, not offered) · §1.5 · G22 G31
  needs: P7.36, P7.37, P7.38
  > a registry/offered-set assertion: `TXT→DOC`, `MD→DOC`, `HTML→DOC` are **NOT offered** (matrix `—` — no everyday `markdown→.doc` demand; the modern `.docx` is the sole Word target for these sources). The bijection guard (§6.4.3a) must not flag a missing fixture/test for a non-offered pair; `*→DOC` is offered ONLY from office sources (P7.40).

---

### Document category: LibreOffice office↔office + DOC→markup

> The fidelity round-trips between `DOCX/DOC/ODT/RTF` and the LO-owned DOC→markup
> down-conversions (pandoc can't read binary `.doc`). LibreOffice for all — keeping
> every pair single-engine. Each pair box `needs:` the LibreOffice runtime wiring.

- [ ] **P7.40** [RUST] Wire `DOC → TXT/MD/HTML` (LibreOffice markup export, NOT pandoc) + the LO-Markdown-export `[DEFER: corpus]` flag · §3.5.2 · G31 G32
  needs: P7.8, P7.10, P7.11, P7.12
  > register `DOC→TXT` (`Text`), `DOC→MD` (`Markdown`†, LO 26.2), `DOC→HTML` (`HTML (StarWriter)`) via **LibreOffice** — pandoc **cannot** read legacy binary `.doc`, so these down-conversions are LO-owned (keeps every pair single-engine, no chaining); lossy `doc_to_text`/`doc_simplified`; LO handles legacy code pages so non-Latin text survives; embedded OLE objects (old equation editor) may not render → reported, not crashed. The LO Markdown EXPORT is new in 26.2 → its reliability is the `[DEFER: corpus]` flag (design fixed, reliability empirical — distinct from the MD-import gate P7.65).
- [ ] **P7.41** [RUST] Wire the office↔office round-trips `DOCX/DOC/ODT/RTF` among themselves (LibreOffice) · §3.5.2 · G31 G32
  needs: P7.8, P7.10, P7.11, P7.12
  > register every office↔office pair via LO with the fixed filters: `→DOCX` (`MS Word 2007 XML`), `→DOC` (`MS Word 97`), `→ODT` (`writer8`), `→RTF` (`Rich Text Format`); the matrix `✓` set (`DOCX↔DOC/ODT/RTF`, `DOC↔DOCX/ODT/RTF`, `ODT↔DOCX/DOC/RTF`, `RTF↔DOCX/DOC/ODT`); `→DOC/ODT` near-lossless (`—`/minor feature loss), `→RTF` `✓` (rich features simplified → `doc_simplified`). ODT round-trips are highest-fidelity (LO home format); fonts/embedded-images/tracked-changes/encoding handled per documents.md edge cases; macros never executed (the P7.5 profile). `*→DOC` offered ONLY from office sources (not TXT/MD/HTML).

---

### Document category: every `*→PDF` (LibreOffice, the platform-wide PDF producer)

> PDF is documented canonically in documents.md and is the default target for every
> document source except PDF itself. ALL `*→PDF` (this category + the cross-category
> spreadsheet/presentation producer rows) go through LibreOffice — it lays out and
> exports in one pass (no chained pandoc→LaTeX). `TXT→PDF` is faithful; the
> word-processor + `MD→PDF` + `HTML→PDF` paths are reflow/render lossy.

- [ ] **P7.42** [RUST] Wire `DOCX/DOC/ODT/RTF → PDF` (LibreOffice `writer_pdf_Export`) + the `doc_pdf_reflow` lossy flag · §3.5.2 §2.9 · G31 G32
  needs: P7.14, P7.41
  > register the four word-processor `→PDF` producers via LO Writer filter `writer_pdf_Export` with the FilterData defaults (P7.25-doc; `UseTaggedPDF=true` — Writer emits well-structured heading/paragraph tags); each is the `★` default for its source; all `✓★~` lossy `doc_pdf_reflow` (font-substitution/reflow — the bundled-font set P7.2 minimizes it); embedded images preserved into PDF.
- [ ] **P7.43** [RUST] Wire `TXT → PDF` (LibreOffice, faithful) · §3.5.2 §2.9 · G31 G32
  needs: P7.14
  > register `TXT→PDF` via LO (lays text into pages); the `★` default for TXT; **faithful (`✓★`, NOT lossy)** — plain text has no structure to reflow, so `TXT→PDF` is the one `→PDF`-via-LO path that carries NO lossy note (deliberately unlike `MD→PDF`).
- [ ] **P7.44** [RUST] Wire `HTML → PDF` (LibreOffice HTML import filter) + the `doc_html_render` lossy flag · §3.5.2 §2.9 · G31 G32
  needs: P7.14
  > register `HTML→PDF` via LO's HTML import filter rendering to a laid-out PDF in one pass (no headless-Chromium/wkhtmltopdf — keeps the bundle lean + the pair single-engine); the `★` default for HTML; lossy `doc_html_render` ("may look different from a web browser" — LO's HTML/CSS engine is not a full modern browser); JavaScript never executed; external remote CSS/images not fetched (offline) → noted gaps; relative local assets resolved; `<meta charset>`/BOM honored; embedded `<svg>`/data-URI images render, remote `<img src=http…>` do not.
- [ ] **P7.45** [RUST] Wire the PDF-export internal fixed-defaults table (none surfaced for documents v1) · §1.6 · G31
  needs: P7.14
  > the documents.md internal fixed defaults passed to the export filter, NONE surfaced ("it just works", Principle 8): `SelectPdfVersion=0` (PDF 1.7, max compatibility — verified `15/16/17` are plain PDF versions NOT PDF/A, the PDF/A levels are `1/2/3`); `UseTaggedPDF=true`; `ReduceImageResolution=false`; `Quality=90`; `ExportBookmarks=true`; page range = all. NO advanced-options panel ships for documents v1; the "compress/smaller PDF" toggle is `[DECIDED]` out of v1 (`[DEFER: post-v1]`).

---

### Document category: PDF→TXT + the canonical producer-list assertion + edge cases

- [ ] **P7.46** [RUST] Wire `PDF → TXT` registration (poppler) as the only PDF-source pair + the parked-reverse rule · §3.5.3 §1.5 · G31 G32
  needs: P7.19, P7.20
  > register `PDF→TXT` (poppler) as the **only** offered PDF-source pair (the `★` default for PDF — "get the text out"); heavily lossy `doc_pdf_to_text`; `PDF→DOCX/ODT/HTML/MD/RTF/DOC` are out of v1 (reverse/reconstructive, SSOT Direction & shape rule) — a registry/offered-set assertion that they are NOT offered (the bijection guard must not flag them); OCR of scanned/image PDFs explicitly Parked.
- [ ] **P7.47** [TEST] Assert the single canonical PDF as-target producer list (all 13 producers in one table) · §1.5 · G22 G31
  needs: P7.42, P7.43, P7.44, P7.52, P7.64
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P7.52` (the spreadsheet→PDF wiring) and `P7.64` (the presentation→PDF wiring) point at later document-order boxes — the 13-producer table cannot be asserted complete until all 13 producers register, so DECISION C builds the spreadsheet/presentation→PDF producers first; the edges are acyclic + valid, documented here at the `needs:` line.
  > a structural assertion that the offered PDF as-target producer set == the canonical documents.md table exactly: the 7 document producers (`DOCX/DOC/ODT/RTF/TXT/MD/HTML→PDF`) + the 3 presentation producers (`PPTX/PPT/ODP→PDF`, P7.64) + the 3 spreadsheet producers (`XLSX/XLS/ODS→PDF`, P7.52) — every PDF producer in the entire app is in this one table; the cross-category rows are owned by their files but the PDF column is asserted assembled in one place (it can never be split or contradicted).
- [ ] **P7.48** [TEST] Assert the document edge cases — encrypted PDF, scanned-PDF empty extraction, encrypted/macro office, fonts · §2.8 · G31
  needs: P7.46, P7.42, P7.40
  > the documents.md edge cases as fail-clearly/no-harm tests: password-protected/encrypted PDF → §2.8 "password-protected" (never cracked, never silent empty); scanned/image-only PDF → near-empty TXT reported honestly; encrypted office files → §2.8 password-protected via the LO exit-0-but-wrote-nothing rule (P7.12); macro-bearing `.docm` never executes a macro (P7.5 profile); a document font neither embedded nor on the system → LO substitutes from the bundled set (P7.2) → minor reflow (`doc_pdf_reflow`), non-Latin never tofus.

---

### Document category: corpus, per-pair tests & lossy-disclosure map

- [ ] **P7.49** [TEST] Stage the document corpus (one file per source format) + manifest + SHA-256 entries · §6.4.5 · G24a G22
  needs: P7.1, P7.17, P7.23, P0.5.11
  > add `tests/corpus/documents/` files: one per source format (PDF text + a scanned/image-only PDF + an encrypted PDF; DOCX; legacy binary DOC; ODT; RTF; TXT; MD with gfm features + local + remote image refs; single-file HTML), each with a root-`manifest.toml` `[[file]]` (source / redistributable licence / `exercises` / `covers` 2-tuples / `[file.expect]`); regenerate the §6.4.5/P0.5.4 SHA-256 corpus manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). Files must be CC0/public-domain/self-produced/synthetic. (`needs: P0.5.11` for the manifest generator.)
- [ ] **P7.50** [TEST] Stage the document content-floor + edge-case fixtures (CJK/RTL, fonts, decompression bombs, malformed) · §6.4.5 §6.4.2 · G24a G31 G48
  needs: P7.49, P0.5.11
  > add the content-floor + edge fixtures: CJK + RTL (Arabic/Hebrew) text documents (the §2.10 content-fidelity corpus — the bundled-font fidelity gate, P7.2); a non-embedded-font document (substitution/reflow); the ZIP-bomb-in-OPC DOCX + a deeply-nested PDF flate-stream decompression-bomb fixture (the §6.4.2/P0.5.3 bomb corpus, fed to the highest-privilege parsers); corrupt/truncated + 0-byte PDF/DOCX/DOC; a `.docx` that is really an ODS (mis-named, content-over-name detection); tracked-changes/comments + embedded-image + embedded-OLE fixtures. These are NEW SHA-256-manifest-tracked fixtures, so regenerate the manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
  - [ ] **P7.50.1** [TEST] Instantiate the P0.4.3 zip-slip archive-entry-name `cargo-fuzz` target over the bounded in-core OPC/ZIP container parse + the `../../etc/passwd`-entry fixture · §1.2 §6.4.2 · G48
    needs: P0.4.3, P7.30, P0.5.11, P3.87
    > the activation target for the **P0.4.3 zip-slip archive-entry-name fuzz leg** — the one P0.4.3 G48 in-core target that had no instantiating box (its peers activate as the instrumented-nightly legs in **P9.35** detect, **P3.73** fs_guard + CSV/TSV, **P4.35.1** imgworker-FFI — with **P3.67** the STABLE-toolchain replay across all in-core targets, `NO libFuzzer harness`, not itself a `cargo-fuzz` activation leg; plus the P2 IPC serde/numeric-overflow legs (**P2.126**), which are G16 `proptest`s in `tests/`, NOT instrumented-nightly libFuzzer legs; the zip-slip leg was orphaned). Stand up the `cargo-fuzz` target over the **bounded, memory-safe in-core OPC/ZIP container-parse** P7.30 builds (the `[Content_Types].xml`/`mimetype` peek that classifies DOCX/XLSX/PPTX/ODF — pure Rust, NO third-party C/C++ decoder pre-detect, §2.12.4) and over the archive **entry-NAME** path-resolution: a `../../etc/passwd`-entry / absolute-path-entry / NUL-in-name fixture must NEVER resolve outside the bounded parse window (the entry name is classified/peeked, never written to a derived FS path), no panic/abort, the decompression-ratio/zip-slip caps (§6.4.2/P0.5.3, the P3 fs_guard `is_safe_output` predicates) actually fire. Date-pinned nightly (Linux/macOS) + the committed crash-corpus replayed via the P3.67 stable-toolchain replay (`crate::fuzz_replay` — the 2026-07-20 homing ruling); the `../../etc/passwd`-entry fixture is SHA-256-manifest-tracked (regenerate via `stage-corpus`, G24a). This is the P7 box the P0.4.3 zip-slip edge points at (`needs: P0.4.3`, the harness contract `[x]` before the loop; `P7.30` for the OPC parser it fuzzes; `P0.5.11` for the manifest generator; `P3.87` for the `convertia-core` lib target its fuzz crate imports — the 2026-07-21 P3.73 fork ruling, temporally satisfied since P3 closes first). → activates the P0.4.3 zip-slip target. (The cross-phase `needs: P0.4.3`/`P7.30`/`P3.87` edge is also declared in the P7.77 reconciliation box.)
- [ ] **P7.51** [TEST] Add the document per-pair integration tests (every doc pair, structural readers per target) + lossy-disclosure map · §6.4.3 §6.5 §2.9 · G31 G32
  needs: P7.49, P7.50
  > for every enumerated document `(source → target)` pair, against every corpus file of its source, on all three platforms: completes (LO exit-0-but-wrote-nothing verified by non-empty output, P7.12); output validated by the MANDATORY structural reader (poppler-opens for PDF + the document→image-or-text OCR `expected_text` content check, NOT a size floor; `unzip`+`[Content_Types].xml` for DOCX; a real reader for ODT/RTF; non-empty/output≠input/size-plausibility for TXT/MD/HTML); no-harm (source `sha256` unchanged, atomic write, no-clobber); fail-clearly on the known-bad fixtures; the §2.9 lossy map fires IFF the §04 matrix flags the pair (`doc_pdf_reflow`/`doc_pdf_to_text`/`doc_to_text`/`doc_simplified`/`doc_html_render`; `TXT→PDF` and `MD→HTML/office` NOT flagged; `MD→PDF` IS flagged `doc_pdf_reflow`); CJK/RTL content-fidelity spot-checks (the §6.4.3 runner is P4-built). G32 lossy-disclosure property holds over the `FormatId×FormatId` product.

---

### Spreadsheet category: LibreOffice workbook pairs + the native CSV↔TSV broadening

> XLSX/XLS/ODS reads & writes + CSV/TSV when a workbook is on a side go through
> LibreOffice; the **native CSV↔TSV pair is already built in P3** (only its workbook
> grouping/registration is surfaced here). The number/date auto-recognition trap is
> defended by the import FilterOptions + the "quoted fields are text" switch. Each
> LO pair box `needs:` the LibreOffice runtime wiring.

- [ ] **P7.52** [RUST] Wire `XLSX/XLS/ODS → PDF` (LibreOffice `calc_pdf_Export`) + the spreadsheet-side `→PDF` options + `doc_pdf_reflow` flag · §3.5.2 §2.9 · G31 G32
  needs: P7.14, P7.10, P7.11, P7.12
  > register the three spreadsheet `→PDF` producers via LO Calc filter `calc_pdf_Export` (PDF target owned by documents.md; these are the producer rows); lossy `doc_pdf_reflow` (live workbook → frozen page; formulas freeze to values, wide tables scale/clip, fonts substitute); the spreadsheet-side controls (Advanced): **Sheets to print** = all non-empty sheets (PDF can be multi-page → all populated sheets, empty skipped); **Page orientation** = inherit doc print settings else portrait; **Fit wide sheets** = fit-to-width 1 page wide (the "why is half my table missing" fix).
- [ ] **P7.53** [RUST] Wire the LibreOffice workbook↔workbook target registrations (three independent filter registrations) · §3.5.2 §2.9 · G31 G32
  needs: P7.8, P7.10, P7.11, P7.12
  > register the workbook↔workbook pairs via LO as **three independent filter registrations** (distinct engine invocation + distinct lossiness analysis each, split into sub-boxes so a failure is attributable). Shared edge rules across all three: charts/images/pivots/conditional-formatting/styles/comments/named-ranges/print-areas preserved workbook↔workbook to the extent both formats support; macros/VBA always dropped, never executed; `.xlsm`/`.fods` detected as XLSX/ODS-family.
  - [ ] **P7.53.1** [RUST] Wire `* → XLSX` (`Calc MS Excel 2007 XML`, the `★` default for XLS/ODS/CSV/TSV) · §3.5.2 · G31 G32
    > `→XLSX` (`Calc MS Excel 2007 XML`) — the `★` default for XLS/ODS/CSV/TSV ("modernise/turn-into-a-real-workbook"); not lossy as a target (the richest workbook container).
  - [ ] **P7.53.2** [RUST] Wire `* → XLS` (`MS Excel 97`) + the `xls_legacy_limits` lossy disclosure (the only lossy workbook target) · §3.5.2 §2.9 · G31 G32
    > `→XLS` (`MS Excel 97`) — the **only lossy workbook target**, the sole carrier of `xls_legacy_limits` (65 536 rows × 256 cols, post-2003 features dropped); the lossy note fires only on this direction.
  - [ ] **P7.53.3** [RUST] Wire `↔ ODS` (`calc8`) — practically lossless for ordinary content · §3.5.2 · G31 G32
    > `→ODS` (`calc8`) — practically lossless for ordinary content (LO home format, no §2.9 note for ordinary content).
- [ ] **P7.54** [RUST] Wire `CSV/TSV → XLSX/XLS/ODS` (LibreOffice import with sniffed-delimiter/encoding FilterOptions) + the CSV-injection-safe import · §3.5.2 §0.11 · G31 G32
  needs: P7.8, P7.10, P7.11, P7.12, P3.28
  > register `CSV/TSV → XLSX/XLS/ODS` via the LO Calc import filter `Text - txt - csv (StarCalc)` with explicit import `FilterOptions` carrying the **P3-sniffed delimiter (token 1) + encoding (token 3)** so LO does not re-guess (deterministic import); **CSV-injection-safe**: a leading `=`/`+`/`-`/`@` cell is imported as **text**, never auto-executed as a live formula (formula evaluation on import NOT exposed in v1 — the DDE/CSV-injection class closed); `CSV→workbook` is **not lossy** (text in, richer container out — only adds structure); ragged rows padded with empty cells (never truncated); a stray BOM consumed not emitted as a phantom first cell; embedded newlines in quoted fields preserved (RFC-4180).
- [ ] **P7.55** [RUST] Surface the native `CSV↔TSV` pair registration in the spreadsheet category (P3-built engine, not rebuilt) · §3.5.6 §3.2 · G31 G32
  needs: P3.41, P3.42
  > register the native `CSV→TSV` and `TSV→CSV` pairs (`EngineProgram::InProcessNative`, the P3-built MIT streamed transform — encoding-normalise → delimiter-swap → RFC-4180 re-quote → CSV-injection literal-preservation) into the spreadsheet category's offered set; **not rebuilt** — only its category membership + per-source-default routing (CSV's offered set includes `TSV` native, TSV's includes `CSV` native) surfaced here. `CSV↔TSV` is **not lossy** (both plain text; only delimiter + encoding normalise to UTF-8). The split keeps CSV↔TSV out of LO precisely to avoid LO's number/date auto-recognition mangling (`0123`→`123`, `3/4`→a date) — a content-fidelity decision, not just perf.

---

### Spreadsheet category: shared option sets, encoding/delimiter policy, multi-sheet & advanced-option declarations

- [ ] **P7.56** [RUST] Wire the CSV/TSV EXPORT FilterOptions assembly (field-sep / quote / encoding / BOM / values-not-formulas) · §3.5.2 · G31
  needs: P7.8
  > assemble the LO `Text - txt - csv (StarCalc)` export `FilterOptions` token string for `workbook→CSV/TSV`: field-separator fixed by target (CSV=ASCII 44, TSV=ASCII 9, token 1); text-delimiter double-quote (ASCII 34, token 2, RFC-4180 quoting); output encoding default **UTF-8** (token 3=76), Windows-1252 (token 3=1)/UTF-16/ISO-8859-1/-15 overrides; BOM off for UTF-8 (token 14, on-request only); **cell content = values as shown** (token 9 *Save cell contents as shown*=true, token 10 *Export cell formulae*=false — a CSV of results not `=A1+B1`). The opposite (export formulas) flips tokens 9/10 (Advanced, off).
- [ ] **P7.57** [RUST] Wire the `* → CSV/TSV` lossy disclosure + the multi-sheet ONE-sheet behaviour + the single-sheet fast path · §3.5.2 §2.9 · G31 G32
  needs: P7.53, P7.56
  > register `XLSX/XLS/ODS → CSV` (CSV the `★` default for XLSX) and `→ TSV` via LO; lossy `sheet_to_delimited` (one sheet only; formatting/formulas-as-text/charts/colours/multi-sheet dropped — values only); the load-bearing multi-sheet decision: a multi-sheet workbook → CSV/TSV exports **ONE sheet** (aligns with the one-source→one-target rule; one-to-many fan-out is parked) with a passive note ("only one sheet is exported to CSV"); single-sheet workbooks get **no note and no picker** (the overwhelming common case, the fast path). `CSV/TSV→PDF` is **out** (matrix note ² — a delimited text file has no page layout; the in-app path is `CSV→XLSX` first) — assert it is NOT offered.
- [ ] **P7.58** [UI] Register the multi-sheet picker advanced-option DECLARATION (default = active sheet, shown only when >1 sheet) · §1.6 · G47
  needs: P7.57, P4.64, P4.74
  > register the `[DECIDED]` (c) multi-sheet picker against the P4 panel: a dropdown shown **only when >1 sheet** is detected, **defaulting to the active sheet** (does not violate "no required choices" — it defaults); silently exporting a sheet the user didn't mean is the data-surprise the SSOT *Fail clearly* spirit dislikes. (The §6.6 usability confirmation of the affordance is the only residual.) (`needs: P4.64/P4.74` — the P4 OptionsPanel widget dispatch + AdvancedDrawer this declaration renders against, per the P7.77 reconciliation obligation.)
- [ ] **P7.59** [RUST] Wire the CSV/TSV encoding + delimiter detection policy + the no-silent-transliteration rule · §1.2 §2.10 · G31 G32
  needs: P3.27, P3.28
  > surface the spreadsheets.md encoding policy onto the office path (the sniff itself is P3-built): on read BOM-first → strict UTF-8 → **Windows-1252** fallback (NOT ISO-8859-1 — Latin-1 mis-handles the `0x80–0x9F` curly-quote/em-dash/€ range); delimiter sniff over the first 20 non-empty lines among `,`/`;`/`\t`/`|` choosing the most consistent field-count (semicolon-CSV handled — `1,50` not mis-split; a tab winner reclassifies as TSV, §1.3); on write **UTF-8 without BOM** default; **no silent transliteration** — characters un-representable in a user-chosen non-Unicode output encoding are flagged lossy (`text_encoding_narrowed`), never dropped/`?`-replaced silently; undetectable/ambiguous → decline clearly (§2.8), never a wrong split.
  - [ ] **P7.59.1** [UI] Register the CSV/TSV input encoding + input delimiter Advanced-option DECLARATIONS · §1.6 · G47
    needs: P4.64, P4.74
    > input encoding (Auto-detect default; UTF-8/UTF-16 LE/BE/Windows-1252/ISO-8859-1/-15 overrides, passed verbatim as LO import token 3 so LO does not re-sniff); input delimiter (Auto-detect default; comma/semicolon/tab/pipe/custom-single-char overrides, passed as token 1). (`needs: P4.64/P4.74` — the P4 OptionsPanel widget dispatch + AdvancedDrawer these declarations render against, per the P7.77 reconciliation obligation.)
  - [ ] **P7.59.2** [UI] Register the "Quoted fields are text" + the output encoding/BOM/export-formulas Advanced-option DECLARATIONS · §1.6 · G47
    needs: P4.64, P4.74
    > "Quoted fields are text" (default OFF — the `0123`-leading-zero / `3/4`-becomes-a-date fix: token "quoted field as text"=true, *Detect special numbers*=false); output encoding (UTF-8 default) + BOM (off for UTF-8) + "export formulas instead of values" (Advanced, off) for the export side; first-row-is-header NOT exposed (a downstream concern). (`needs: P4.64/P4.74` — the P4 OptionsPanel widget dispatch + AdvancedDrawer these declarations render against, per the P7.77 reconciliation obligation.)
- [ ] **P7.60** [TEST] Wire the spreadsheet per-source-default-target table (XLSX→CSV; XLS/ODS/CSV/TSV→XLSX) zero-click assertion · §1.5 §1.6 · G31 G61
  needs: P7.52, P7.53, P7.55, P7.57, P4.60.2
  > the per-CATEGORY spreadsheet default-target table: the pre-highlighted default = **CSV** for XLSX, **XLSX** for XLS/ODS/CSV/TSV; its §04-offered spreadsheet pairs + their `OptionDecl` defaults FEED the §1.6 consolidated defaults registry the **P4.60.2 G61 guard** merges + checks across all options of all offered pairs (the single machine-checkable home of the no-required-choices gate — `needs: P4.60.2` so the spreadsheet default table is registered against the guard, not asserted ad-hoc here). XLSX→CSV is the one debatable call (`[DEFER: corpus]` §6.6 usability confirmation — a measured call, not an open design question). Pipe-delimited `.psv` is auto-DETECTED as a CSV input variant, never offered as a target (`[DECIDED]` NOT in v1).

---

### Spreadsheet category: corpus, per-pair tests

- [ ] **P7.61** [TEST] Stage the spreadsheet corpus + content-floor/edge fixtures + manifest + SHA-256 · §6.4.5 §6.4.2 · G24a G22 G31
  needs: P7.1, P0.5.11
  > add `tests/corpus/spreadsheets/` files: one per source (XLSX multi-sheet + formulas + charts + `.xlsm` + **a workbook with ≥1 hidden column AND ≥1 hidden sheet** for the §spreadsheets hidden-data → CSV used-range case; legacy XLS; ODS + `.fods`; CSV — comma + semicolon-European + Windows-1252-encoded + a leading-`=`/`@` CSV-injection file + a quoted-embedded-delimiter file + a CJK/RTL-content file; TSV with an in-field tab); edge fixtures (a >65 536-row workbook for `xls_legacy_limits`; an encrypted XLSX; a `WEBSERVICE`/external-data-range `.xlsx` T9b sentinel; a ragged-row CSV; a multi-sheet workbook for the picker); each a `manifest.toml` `[[file]]` + redistributable licence; regenerate the SHA-256 manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (Reuses the P3 CSV/TSV fixtures where they already cover a case — no inline duplication, single-source helper.) (`needs: P0.5.11` for the manifest generator.)
- [ ] **P7.62** [TEST] Add the spreadsheet per-pair integration tests (every sheet pair, structural readers) + the CSV-injection/value-fidelity checks · §6.4.3 §6.5 §2.9 · G31 G32
  needs: P7.61
  > for every enumerated spreadsheet `(source → target)` pair, against every corpus file of its source, on all three platforms: completes (LO exit-0-but-wrote-nothing verified); output validated by the structural reader (`unzip`+`[Content_Types].xml`+`xl/workbook.xml` for XLSX; ODF reader for ODS; a real RFC-4180 CSV/TSV reader — NOT bare field-count — with the CSV-injection literal-preservation assertion and the no-value-mangling `0123`/`3/4` checks; poppler-opens for PDF); no-harm + fail-clearly (encrypted/corrupt fixtures); the multi-sheet single-sheet-export note fires on >1-sheet sources only; **on the hidden-column/sheet workbook → CSV/TSV (single sheet), assert the output is the sheet's USED RANGE with hidden columns EMITTED AS DATA** (the §spreadsheets Edge case — hidden data is included, not stripped, on a delimited single-sheet export); `sheet_to_delimited`/`xls_legacy_limits`/`text_encoding_narrowed`/`doc_pdf_reflow` fire IFF the §04 matrix flags it; `CSV↔TSV` + `CSV/TSV→workbook`(UTF-8) carry NO note; CJK/RTL value-fidelity spot-checks.

---

## Internal §6.5 sub-gate — spreadsheets reliable before the document/markup risk

- [ ] **P7.63** [TEST] Sub-gate — assert every spreadsheet pair `reliable` in the ledger (the simplest LO-only category) · §6.5 §6.5.2 · G31 G32
  needs: P7.60, P7.62
  > the intra-phase milestone: assert the §6.5.2 pair-status ledger (`reliability-report.json`) marks EVERY enumerated spreadsheet pair `reliable` on all three platforms before the document category (whose pandoc/poppler engines + the `MD→PDF` ship-or-park gate add the remaining risk) is hardened — spreadsheets reuse the most P3-built machinery so they give the earliest measurable progress. The named checkpoint between the LO-workbook cluster and the multi-engine document cluster.

---

### Presentation category: LibreOffice slide pairs (PPTX/PPT/ODP + →PDF)

> A small single-engine (LibreOffice) category: `PPTX/PPT/ODP` office↔office + the
> `→PDF` producer rows. PDF default for all three. The two MS-family directions are
> ASYMMETRIC: `PPT→PPTX` plain `✓` (modernizing), `PPTX→PPT` lossy `pptx_to_ppt_legacy`
> (downgrade can't store SmartArt/modern-charts/Morph). ODF↔MS always lossy. Each
> pair box `needs:` the LibreOffice runtime wiring. (PDF→PPTX/ODP reverse parked;
> slide→image fan-out parked.)

- [ ] **P7.64** [RUST] Wire `PPTX/PPT/ODP → PDF` (LibreOffice `impress_pdf_Export`) + the `slides_to_pdf_flatten` lossy flag + Impress `UseTaggedPDF=false` · §3.5.2 §2.9 · G31 G32
  needs: P7.14, P7.10, P7.11, P7.12
  > register the three slide `→PDF` producers via LO Impress filter `impress_pdf_Export`, the `★` default for every slide source; lossy `slides_to_pdf_flatten` (editability lost; animations/transitions/triggers flattened to final state; embedded video/audio dropped — poster/first-frame only; fonts substituted if not embedded; speaker notes omitted unless the notes switch is on); `UseTaggedPDF=false` deliberately (Impress tagged-PDF is limited/noisy — the intentional asymmetry vs documents' Writer `UseTaggedPDF=true`, not a harmonisation gap). Container-detection collisions (P7.30/P7.31) are the headline risk.
- [ ] **P7.65** [RUST] Wire the slide office↔office pairs with the ASYMMETRIC MS-family loss (`PPT→PPTX` plain · `PPTX→PPT` lossy) + ODF↔MS lossy · §3.5.2 §2.9 · G31 G32
  needs: P7.8, P7.10, P7.11, P7.12
  > register the slide office↔office pairs via LO: `→PPTX` (`Impress MS PowerPoint 2007 XML`), `→PPT` (`MS PowerPoint 97`), `→ODP` (`impress8`); the [OPEN-1]-resolved asymmetry — **`PPT→PPTX` plain `✓`** (modernizing to a richer format that holds everything the legacy one did → NO §2.9 note) vs **`PPTX→PPT` `✓~` lossy `pptx_to_ppt_legacy`** (downgrade to BIFF8/PowerPoint-97 structurally can't store SmartArt/modern-charts/Morph → simplified/dropped); the cross-model `ODP→PPTX/PPT` + `PPTX/PPT→ODP` all `✓~` `office_roundtrip_approx` (ODF↔MS shapes/styles/transitions approximated); same-format cells `—` (not offered, degenerate — no "re-compress" use case for slides); embedded media/fonts/OLE + macros-dropped per the per-format edge cases; ODP is the highest-fidelity import (LO home format).
- [ ] **P7.66** [TEST] Assert the slide reverse/fan-out parked rules (PDF→PPTX/ODP out; slide→image out) + the no-office-office-options rule · §1.5 · G22 G31
  needs: P7.64, P7.65
  > a registry/offered-set assertion: `PDF→PPTX/PPT/ODP` are out (reverse/reconstructive, parked — the bijection guard must not flag them); slide→image fan-out (one PNG/JPG per slide) is parked (one-to-many, SSOT direction rule — LO `impress_png_Export`/`impress_jpg_Export` recorded as a clean post-v1 add, capability noted not lost); office→office presentations expose **no** options at all (a straight engine-default re-encode — intentional, not a gap).
- [ ] **P7.67** [UI] Register the "Include speaker-notes pages" Basic-option DECLARATION (`ExportNotesPages=true`) · §1.6 · G47
  needs: P7.64, P4.64, P4.74
  > register the single Basic switch for the slide `→PDF` pairs against the P4 panel: "Include speaker-notes pages" → **`ExportNotesPages=true`** (notes PAGES, the full-page layout — NOT `ExportNotes=true` notes-as-annotations, [OPEN-3] resolved), default OFF; the one switch with broad everyday demand ("export the deck with my notes for the printout"). The rest of the impress PDF FilterData (Quality/ReduceImageResolution/MaxImageResolution/UseLosslessCompression/SelectPdfVersion/EmbedStandardFonts) stay Advanced/not-exposed at engine defaults; office→office has no exposed options. (`needs: P4.64/P4.74` — the P4 OptionsPanel widget dispatch + AdvancedDrawer this declaration renders against, per the P7.77 reconciliation obligation.)

---

### Presentation category: corpus, per-pair tests

- [ ] **P7.68** [TEST] Stage the presentation corpus + content-floor/edge fixtures + manifest + SHA-256 · §6.4.5 §6.4.2 · G24a G22 G31
  needs: P7.1, P0.5.11
  > add `tests/corpus/presentations/` files: one per source (PPTX with SmartArt + modern charts + a Morph transition + embedded media + embedded fonts + speaker notes; legacy binary PPT + a `.pps`; ODP + a `.otp`; macro-enabled `.pptm`/`.ppsx`), each a `manifest.toml` `[[file]]` + redistributable licence; edge fixtures (an encrypted/password PPTX; a corrupt/partial OPC zip; a 0-byte deck; a CFB-ambiguous `.ppt`-vs-`.doc`-vs-`.xls` set; a CJK/RTL-text deck; a deck whose font is not embedded; a deck with embedded video for the poster-only drop); regenerate the SHA-256 manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
- [ ] **P7.69** [TEST] Add the presentation per-pair integration tests (every slide pair, structural readers) + the asymmetric-loss assertion · §6.4.3 §6.5 §2.9 · G31 G32
  needs: P7.68
  > for every enumerated slide `(source → target)` pair, against every corpus file of its source, on all three platforms: completes (LO exit-0-but-wrote-nothing verified); output validated by the structural reader (poppler-opens + the slide→image OCR `expected_text` content check for `→PDF`; `unzip`+`[Content_Types].xml` for PPTX; ODF reader for ODP); no-harm + fail-clearly (encrypted/corrupt/0-byte fixtures); macros never executed; the asymmetric loss asserted (`PPT→PPTX` fires NO note; `PPTX→PPT` fires `pptx_to_ppt_legacy`; ODF↔MS fires `office_roundtrip_approx`; `→PDF` fires `slides_to_pdf_flatten` unconditionally); embedded-media poster-only drop + font-substitution + CJK/RTL content-fidelity spot-checks. G32 lossy-disclosure-iff-flagged holds.

---

### The LibreOffice Markdown-import ship-or-park gate (the load-bearing `MD→PDF` decision)

> The single load-bearing reliability decision of the phase: native LibreOffice
> Markdown *import* landed only in LO 26.2 (Mar 2026) and is unproven. `MD→PDF` has
> **NO chain-free fallback** (the `MD→pandoc-HTML→LO-PDF` chain is explicitly
> disallowed, §3.2) — so if the LO 26.2 import corpus gate FAILS, `MD→PDF` is
> DEMOTED TO PARKED (per the SSOT v1-DoD second exception), never silently chained.
> `MD→DOCX/ODT/RTF/HTML/TXT` are pandoc-owned and unaffected (P7.37).

- [ ] **P7.70** [RUST] Wire `MD → PDF` via LibreOffice 26.2 Markdown import + the `doc_pdf_reflow` lossy flag (the ship path) · §3.5.2 §2.9 §3.2 · G31 G32
  needs: P7.14, P7.42
  > register `MD→PDF` via **LO 26.2's native Markdown import** → `writer_pdf_Export` (single-engine, no chaining); the `★` default for MD; lossy **`doc_pdf_reflow`** (the one MD→PDF exception — LO lays Markdown out with font-substitution/reflow exactly like the word-processor `→PDF` paths, classified the SAME as DOCX/DOC/ODT/RTF/HTML→PDF, **not** "faithful" like the structureless `TXT→PDF`). Local relative images resolved; remote URLs not fetched (offline) → broken refs noted. This is the ship path that the P7.71 gate either confirms reliable or demotes.
- [ ] **P7.71** [TEST] Run the LO-26.2-Markdown-import corpus gate → resolve ship-vs-park for `MD→PDF` (record in `docs/demoted-pairs.md` if parked) · §6.5 §6.5.3 §3.2 · G31 G32
  needs: P7.70, P7.49, P7.50
  > the `[DEFER: corpus]` resolution: run the LO 26.2 Markdown-import path against the MD corpus (gfm features, local + remote image refs, code blocks, front-matter, CJK/RTL) on all three platforms and decide: **(ship)** if it meets the §6.5 reliability bar → `MD→PDF` is `reliable` in the ledger; **(park)** if it FAILS → `MD→PDF` is **demoted to parked** (SSOT v1-DoD second exception) — NOT chained (the `MD→pandoc-HTML→LO-PDF` chain stays disallowed, §3.2), NOT shipped broken — with a `docs/demoted-pairs.md` row (kind=`reliability-demotion`, affected platform(s), reason, ledger ref) + the §6.5.3 release-note mirror + the §6.8 orphan-row check. Phase 3 must NOT assume a silent fallback exists.
- [ ] **P7.72** [TEST] Resolve the `MD→ODT/DOCX/RTF` LO-import-vs-pandoc ownership (`[DEFER: corpus]`, single registry owner either way) · §6.5 §3.2 · G31 G32
  needs: P7.71, P7.37
  > the documents.md open-item 1 second half: the v1 default is LO 26.2 imports `.md` and exports ODT/DOCX directly (single-engine); the documented fallback **only if the corpus shows LO MD import unreliable** is pandoc owning `MD→DOCX/ODT/RTF/HTML/TXT` (P7.37 already wires the pandoc path). Resolve against the same P7.71 corpus run; whichever way it lands the registry stays a SINGLE owner per pair (the trait/lookup unaffected, §3.2.3). Unlike `MD→PDF`, these DO have the chain-free pandoc fallback so they are never parked. Record the resolution in this plan's notes.

---

### Phase reliability gate, cross-engine T9b egress, advanced-option completeness & engine-bump re-validation

- [ ] **P7.73** [TEST] Add the per-push adversarial-egress + T9b-sentinel PULL-FORWARD leg for the office engines (LO/pandoc/poppler) · §6.4.2 §2.11.4 §0.11 · G42 G42b
  needs: P7.51, P7.62, P7.69, P0.7.12
  > the §6.4.2 per-push adversarial-egress + T9b-sentinel corpus run inside G42's egress-deny window (the **P0.7.12 leg-(b) per-push pull-forward** activating from P6/P7 as the egressing office engines are staged — carries `needs: P0.7.12` mirroring the peer pull-forward boxes P6.44/P6.64 exactly, so the leg-(b) P7 activation edge is verifiable by the plan graph, not only an activator-prose claim): the crafted-`WEBSERVICE`/external-data-range `.xlsx`, the remote-`href`/`<image>` HTML/MD, the remote-OLE/link office doc, and the remote-URI PDF must each show **ZERO egress (incl. zero DNS) AND no out-of-input file read** — so a T9b regression in the LO profile, pandoc `--sandbox`, or poppler is caught on the push that introduces it. This is the release-blocking proof the LO Calc-external/registry pins (P7.6, defence-in-depth) lean on. (Full per-OS deny window + release-confirmation leg are P9.)
- [ ] **P7.74** [TEST] Assert the §6.5 phase reliability gate — every P7 pair `reliable` on all three platforms (with the recorded exceptions) · §6.5 §6.5.2 §6.5.3 · G31 G32
  needs: P7.63, P7.51, P7.69, P7.71, P7.72, P7.73
  > the phase-level §6.5 coverage gate: the §6.5.2 pair-status ledger marks EVERY enumerated P7 pair (all document + spreadsheet + presentation pairs) `reliable` on every platform where it is not `demoted`; any `failing` cell blocks the release; the two permissible exceptions recorded in `docs/demoted-pairs.md` + the ledger with the required fields (`MD→PDF` if P7.71 parked it = the dominant candidate; no office format is patent-gapped so exception 1 does not apply here). The report is published as a release asset (transparency).
- [ ] **P7.75** [TEST] Assert the office-family advanced-option + completeness gates (every declared option resolves + every pair has a test + every format in the matrix) · §1.6 §6.4 · G22 G23
  needs: P7.58, P7.59, P7.67, P7.74
  > the §6.4 completeness wiring for the office engines: **G22** every office format (PDF/DOCX/DOC/ODT/RTF/TXT/MD/HTML/XLSX/XLS/ODS/CSV/TSV/PPTX/PPT/ODP) ∈ the §04 category format matrices (the `docs/spec/04-formats/` documents/spreadsheets/presentations matrices the bijection guard reads — not a README table) ∧ has a corpus fixture ∧ has a round-trip test; **G23** — the conversion-command→test walk (keyed to the §0.4.1 `start_conversion` — the 2026-07-17 P3.63 ruling) stays green, and every office pair has a partner test via the §6.4.3 runner / §6.4.3a `covers` bijection; every registered §1.6 advanced-option declaration (multi-sheet picker, CSV/TSV encoding/delimiter/quoted-text/output, speaker-notes) resolves to a non-empty handler + a UI control on the P4 panel (no orphan declaration, no declared-but-unwired option).
- [ ] **P7.76** [TEST] Add the office-engine bump re-validation hook (full reliability gate re-runs on a LO/pandoc/poppler/font pin change) · §6.5.4 §3.8 · G37 G17b
  needs: P7.3, P7.17, P7.23, P7.74
  > wire the §6.5.4 rule for the LibreOffice / pandoc / poppler / bundled-font `engines.lock` pins: a version/SHA change re-runs the FULL P7 reliability gate before that engine version can ship (a patch must not silently regress a pair — incl. a LO bump that could change the 26.2 Markdown-import behaviour P7.71 gated on); the ledger status-diff is part of the bump review; the informational per-push OSV/grype over the PURL/CPE-keyed LO/poppler rows (poppler/LibreOffice are real CVE surfaces) feeds vuln-response (CVSS ≥ 7 on an exercised office path → release-blocking escalation).

---

### Cross-phase reconciliation (the deferred P7→P4 harness edges)

- [ ] **P7.77** [GATE] Wire the deferred P7→P4 harness reconciliation `needs:` edges — isolation boundary, §1.7 progress/classify, per-pair runner + ledger + bijection, TCC staging, verifier, options-panel shell · §3.5.2 · G7 G20
  needs: P4.8, P4.13, P4.14, P4.24, P4.25, P4.43, P4.49, P4.59, P4.60, P4.61, P4.64, P4.74, P4.28.1, P0.7.3, P0.7.4
  > the P7 instance of the cross-phase reconciliation obligation (the master plan-lint forbidden-string check is P4.77; reciprocal of P3.70/P5.74/P6.92/P9.46): declare the load-bearing P7→P4 + P7→P0 edges the office-family boxes consume — the office staging executes the **P0.7.3 engine-acquisition + allow-list policy** (P7.1/P7.17/the pandoc staging) and the per-engine §6.1.3 assertions execute the **P0.7.4 build-assertion policy** (P7.18 + the LO/pandoc assertion boxes); the from-source poppler built-without-network compile (P7.17.1) fills the **P4.28.1 from-source compilation harness** configure-flag manifest seam (the curated build the P7.18 §6.1.3 assertion can only pass against); every LibreOffice/poppler/pandoc invocation routes through the **P4.13/P4.14 §2.12 isolation wrapper** (P7.4/P7.19/P7.23, inline edges already on those boxes) + handles progress/stderr-classify through the **P4.8 §1.7 line-reader + P4.49 classify seam**; macOS TCC source-staging is **P4.24/P4.25** (the read-side staging for the office sidecars); the per-engine availability rows populate the **P4.43 verifier framework** (P7.17/P7.24, inline); every per-pair test runs on the **P4.59 §6.4.3 runner** (P7.51/P7.62/P7.69 consume it — the deferred edge owned here, no per-box inline) and the phase/sub-gates drive the **P4.60 bijection guard + P4.61 ledger generator** (P7.63/P7.74/P7.75); the zip-slip archive-entry-name `cargo-fuzz` target (P7.50.1) instantiates the **P0.4.3 zip-slip G48 leg** over the P7.30 bounded in-core OPC parse (inline `needs: P0.4.3, P7.30, P3.87` — the P3.87 `convertia-core` lib target its fuzz crate imports, the 2026-07-21 P3.73 fork ruling); every advanced-option DECLARATION box (P7.58/P7.59.1/P7.59.2/P7.67) renders against the **P4.64 OptionsPanel widget dispatch + the P4.74 AdvancedDrawer** (inline edges on those boxes). `needs:` these P4 harness boxes here so the §6 selection builds the P4 mechanism first (P4 is `[x]` before the loop reaches P7 — the edges must RESOLVE, not dangle; the inline engine/declaration edges on P7.4–P7.67 carry the per-box dependency, this box is the auditable single owner for the runner/ledger/bijection edges P7.51/P7.62/P7.69/P7.74 do not inline). No P7 box `>`-note defers a `needs:` with the P4.77-forbidden phrasing.

---

### The phase-end Co-Pilot hardening sweep — the standing phase-close box

> The standing test-strategy §11 phase-close box (owner directive, recorded 2026-07-06):
> Co-Pilot-executed — never the Build-Loop; mandate, level and evidence rules in
> [test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep).

- [!extern] **P7.78** [TEST] Run the phase-end Co-Pilot hardening sweep over the whole P7 delivery — adversarial re-test at the hardest technically-possible level · §6.4
  > **[!extern] (Co-Pilot-executed — the standing test-strategy §11 phase-close sweep, never the Build-Loop):** runs once every other P7 box is `[x]`; the phase's whole delivery is adversarially re-tested at the hardest technically-possible level with unrestricted session tooling (Docker, WebDriver/Playwright, property/fuzz/mutation probes, real-OS live runs); findings are fixed with tests as normal dual-reviewed commits before this box flips `[x]`.
  > **Second leg (§11.4, owner directive 2026-07-22):** the same sweep then pre-fill-audits the P8 plan boxes against the as-built codebase + their cited spec §§ + their dependency edges (incl. embedded-type edges plan-lint cannot see) + information-completeness; resolvable findings land as dual-reviewed plan/spec edits BEFORE the P8 build session starts, genuine forks go to the owner batched at the boundary.
  > **Boundary stop:** P8.1 carries `needs:` on this box — a `[!extern]` prerequisite of a non-extern box is a loop STOP (`_format.md` §2/§6), so the loop hard-stops at the P7→P8 boundary and hands off to the Co-Pilot until the sweep is `[x]`.
