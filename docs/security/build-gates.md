# ConvertIA — Build-Gate Catalogue (living)

> The operational catalogue of every guardrail: **when** it runs, **what** it
> blocks, **which tool** enforces it, and its **fail posture**. Companion to the
> [security-concept.md](security-concept.md) (the *why*). Living document — same
> conflict order and update rules as the concept doc.
>
> Gate IDs are stable (`Gnn`). The plane column maps to the
> [defense-in-depth planes](security-concept.md#3-defense-in-depth--the-enforcement-planes)
> L0–L5.

## 0. Policy

- **Two planes.** Each gate marked *(mirror)* runs both as a local git-hook
  (L1/L2/L3) **and** in CI (L4). Local = realtime; CI = clean-checkout backstop.
- **Performance budgets.** pre-commit (L1) target **< 10 s**; pre-push (L2) target
  **< 3 min**; anything heavier is **CI-only (L4)** or **release-only (L5)**.
- **Hooks run in parallel** within a plane; gates sharing a resource (e.g. one
  test DB-equivalent / one build dir) are merged into a single sequential wrapper.
- **Fail-closed by default.** A gate blocks on doubt. Only the explicitly listed
  gates are **fail-open** (skip when a prerequisite is genuinely absent), and only
  because another plane guarantees enforcement.
- **No bypass.** `--no-verify`, force-push, and disabling a required CI check are
  forbidden (CLAUDE.md). A red gate is fixed, not bypassed.
- **Severity ≠ exit code** for custom gate scripts: *any* finding fails the gate;
  severity only drives output formatting.
- **Custom gate scripts are themselves tested** (positive **and** negative cases —
  every custom gate and fastpath detector ships a self-test that proves it FAILS on
  a planted violation, not only that it passes clean) under a narrowly-scoped
  self-test gate (G24).
- **Structural parsing over regex.** A gate that consumes a generated/structured
  file (JSON/TOML/YAML/lockfile/SBOM) **parses** it (`jq`/`serde_json`/a real YAML
  reader), never a regex — and a gate that **cannot resolve or parse its input fails
  CLOSED in CI** (a missing/unparseable target is a gate failure, not a skip).
- **Offline tolerance.** Any gate with a network step (advisory-DB refresh, rule
  fetch) **decouples** the refresh (warn-only) from the check (hard-fail against
  the local/vendored DB), and honours an offline env flag.
- **The dual review (G1) is a quality amplifier, NOT a security control.** Only the
  deterministic gates G2–G50 are security controls. The `Dual-Review:` trailer is
  self-attested and unverifiable, so a gamed trailer cannot ship insecure code —
  the gates either pass on a clean checkout or they do not.

## 1. L0 — Build-Loop per box (the dual review, "holy grail")

**G1 — Opus + Sonnet pre-commit dual review.** *Plane L0.* Before each build
commit, two model reviewers (`opus` + `sonnet`) critique the **staged diff**
(`git diff --cached`, inline — not a SHA) for completeness, correctness, and
spec-conformance, plus the relevant spec `§§` + this catalogue.
- Output per reviewer: findings ranked **P0→P3**, each with a one-line reason +
  spec-`§`/file ref; convergence/divergence stated explicitly.
- **P0/P1 → fix in the working tree, re-stage, re-review** (loop). **No push
  between fix and re-review** (no fix-push cycle). **P2/P3 →** documented in the
  commit body + a follow-up box if structural.
- Recorded as a machine-readable commit trailer of the **exact** form
  `Dual-Review: opus=GO sonnet=GO` (or `NOGO`) — the *evidence of review* is
  mandatory; each reviewer's findings + convergence/divergence are recorded verbatim
  in the commit body (so a "both GO, 0 findings" on a non-trivial diff is an
  auditable smell for Co-Pilot spot-audit).
- **Staged-diff sanity (the trailer attests *this exact staged diff*).** Immediately
  before `git commit`, `git diff --cached --stat` MUST match the file set the two
  reviewers saw at GO; any file added/removed after GO **requires re-review** (no
  silent post-review staging).
- **Definition-of-Done.** A box is "done" only when it satisfies the ConvertIA DoD
  (authored in [build-loop.md](../process/build-loop.md), P0.6): (a) spec-`§`
  referenced or marked tooling-only; (b) spec synced in the same commit; (c) tests
  at the highest sensible level green; (d) hard gates green with **no** `--no-verify`;
  (e) dual-review done + trailer present; (f) inline `[Build-Session-Entscheidung]`
  tags at non-spec choice sites; (g) `engines.lock` + SBOM row if a new engine was
  staged; (h) §0.11 + security-concept §5 row if a new threat class was introduced.
- **Skipped only** for: (a) check-off commits with no code/config diff; (b)
  `[!extern]` boxes (nothing built).
- *Blocks:* the build commit (self-gate; the trailer is also checked at pre-push by
  G12).

## 2. L1 — pre-commit (cheap, < 10 s, parallel)

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G2** | Secrets / credential scan *(mirror)* | **`gitleaks`** (pinned version) — entropy + curated ruleset covering cloud keys, GitHub PATs (`ghp_`/`github_pat_`), PEM/private-key blocks (catches an accidentally-staged minisign private key), generic high-entropy literals; L1 = `gitleaks protect --staged`, L4 = full-tree scan; committed `.gitleaks.toml` allowlist for test-fixture keys | commit | always-on (no glob); fail-closed |
| **G3** | Format check *(mirror)* | `cargo fmt --check` + `prettier --check` (TS/CSS/JSON; check-only, no auto-write) — Biome's linter, if wanted, belongs in G5, not here | commit | by file glob; fail-closed |
| **G4** | Lint — Rust *(mirror)* | `cargo clippy -D warnings` | commit | changed-crate scope; fail-closed |
| **G5** | Lint — TS/React *(mirror)* | `eslint` (flat config) + `stylelint` (CSS) | commit | by glob; fail-closed |
| **G6** | Type-check (fast) *(mirror full at L2)* | `tsc --noEmit` on changed scope; (`cargo check` is implicit in clippy) | commit | by glob; fail-closed |
| **G7** | Doc-consistency `plan-lint`/`spec-lint` *(mirror)* | custom stdlib-only script (see §6) `--quiet` | commit | on docs glob; any finding = exit 1 |
| **G8** | Deferral / dead-marker gate *(mirror)* | diff-based scan for `TODO`/`FIXME`/`unimplemented!`/`todo!`/`unreachable!`/`dbg!`/`println!`/`console.log`/`": any"`/`as any`/inline `style=`/`style:`/"stub"/"placeholder"/"phase 2" in **new** production lines lacking a box-id or `[!extern]` within ±6 lines (the `any`/inline-CSS markers encode the CLAUDE.md hard rules) | commit | new-marker only; **fail-open** if no diff base (mirror at L4 is **fail-closed**, full-tree, no diff-base excuse) |
| **G9** | Repo-invariant grep gate *(mirror)* | cheap repo-wide regex for a project invariant (e.g. no hardcoded colours outside token primitives), path-whitelisted; the structural CSP/capability invariant is its own gate **G47** (it parses, not greps) | commit | by glob; fail-closed |
| **G10** | Fastpath self-tests | `test-*-fastpath-pattern` smoke tests (script naming convention documented in P0.2; → scripts authored in P0.2) | commit | only when a fastpath detector is edited; positive **and** negative self-tests required |
| **G47** | WebView CSP + capability structural lint *(mirror — L1 cheap, L4 structural)* | parse `tauri.conf.json` `app.security.csp` + `src-tauri/capabilities/*.json` with `jq`/`serde_json` (**not** regex) and FAIL on any §0.10 violation: any **remote** scheme in any CSP directive (the only allowed `connect-src` non-`'self'` tokens are the Tauri IPC `ipc:` + `http://ipc.localhost`; **no `https://` remote, no `asset:`**), `object-src ≠ 'none'`, `form-action ≠ 'self'`, any `fs:`/`http:`/`shell:allow-execute`/`opener:*`/`dialog:` grant in a capability file, or presence of an `updater`/bundle-updater block or updater pubkey in the conf — the concrete instantiation of the G9 invariant placeholder for T2/T2a/T2c/T9a; verifies the §7.6.1 updater-absence claim structurally | commit | always when the conf/capabilities glob matches; fail-closed |
| **G49** | Workflow lint (fast) *(mirror)* | **`actionlint`** — YAML/expression/shell lint of `.github/workflows/*` (catches syntax, bad `${{ }}` expressions, shellcheck issues in `run:` steps) | commit | on `.github/` glob; fail-closed |
| **G51** | Prose typo gate *(mirror)* | **`typos`** (`typos-cli`) — curated-list typo finder (not a dictionary spell-checker; near-zero false positives on Rust/TS identifiers) over public-facing prose: `SECURITY.md`/`PRIVACY.md`/`TRADEMARK.md`, the verify-your-hash recipe, the user-facing error/string catalog — a typo in the security policy or the `minisign` verify recipe is a trust-damaging defect G8/G21 (markers, not prose) miss | commit | on docs/strings glob; fail-closed |
| **G52** | Cross-platform EOL/charset hygiene *(mirror)* | committed **`.editorconfig`** + **`editorconfig-checker`** — EOL/charset/final-newline guard for `.toml`/`.yaml`/`.md`/shell scripts (G3 covers only `cargo fmt` + `prettier`-managed TS/CSS/JSON, leaving these unguarded; a CRLF drift in a `.sh` gate-script/hook is a real Windows footgun) | commit | by glob; fail-closed |

## 3. L3 — commit-msg

> *(Ordering note: L3 is documented here, right after L1, because both fire at
> `git commit` time — the two commit-time planes are grouped; L2 (`git push` time)
> follows in §4. The defense-in-depth plane list in [security-concept.md §3](security-concept.md#3-defense-in-depth--the-enforcement-planes)
> remains in strict L0→L5 order.)*

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G11** | Conventional-commit format | regex `^(feat\|fix\|chore\|docs\|refactor\|test\|perf\|ci\|build)(\([a-z0-9._-]+\))?: .+` (first line); merge/revert/fixup exempt. Solo-on-`main` rollback convention: `chore(scope): roll back — <reason>` (no `revert` type for build-session commits) | commit | always; fail-closed |
| **G12** | Dual-review trailer present + well-formed | the build commit body carries a trailer matching `^Dual-Review: opus=(GO\|NOGO) sonnet=(GO\|NOGO)$` (checked at **pre-push** — the body isn't available at commit-msg time); skipped for check-off/`[!extern]` | push | conditional; fail-closed |

## 4. L2 — pre-push (heavier, < 3 min, parallel; cheap-commit fastpath)

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G13** | Full type-check *(mirror)* | `tsc --noEmit` whole project | push | always; fail-closed |
| **G14** | Full lint *(mirror)* | `clippy --all-targets --all-features -D warnings` + `eslint` whole tree | push | always; fail-closed |
| **G15** | Unit + integration tests *(mirror)* | `cargo test` (incl. real-file round-trips) + `vitest run` | push | always; fail-closed |
| **G16** | Property + fuzz smoke *(mirror)* | property tests — **`proptest`** (Rust, macro-based shrinking, no manual `Shrink` impls — satisfies the §P0.5 "shrinking mandatory" rule) + **`fast-check`** (TS); plus a fast **deterministic** fuzz leg here (a saved-crash-corpus replay / `proptest` smoke over `crate::detect` — **not** an instrumented libFuzzer build, which is L4-only and Unix-nightly-only). The coverage-guided `cargo-fuzz` harness is **G48** (in-core detector) + **G26** (full pass) | push | always; coverage-guided fuzz at L4 |
| **G17** | Dependency-vuln audit *(mirror)* | `cargo audit --locked` + `pnpm audit --audit-level=high` (DB refresh warn-only/offline-tolerant). **NB:** these cover only the Rust crate + npm graph — the bundled-engine CVE surface is **G17b** | push | always; check fail-closed, refresh fail-open |
| **G17b** | Bundled-engine CVE awareness *(informational, release-tier)* | feed `engines.lock` `(component, version)` pairs to **`osv-scanner`** (consumes OSV — indexes FFmpeg/poppler/LibreOffice/x265 advisories) **or** `grype` (consumes the G35 CycloneDX SBOM); emit a dated open-CVE report as a signed-off release asset. **Non-blocking** to honour the SSOT §3.8 "engine currency is best-effort, not a gate" posture; offline-tolerant (vendored DB, refresh warn-only) | informational | report only; never blocks |
| **G18** | License + supply-chain policy *(mirror)* | `cargo deny check` with an **explicit** `deny.toml`: `[bans]` deny-list for **`tauri-plugin-updater`** + the common HTTP-client crates (`reqwest`/`ureq`/`hyper`/`isahc`/`curl`) — no socket-opening dep enters the core (T2/T9a); `[licenses]` GPL/AGPL **denied** for the Rust crate graph (distinct from the bundled-engine aggregation); `[sources]` populated allow-registry/allow-git list (the `sources` check is a no-op without it) | push | always; fail-closed |
| **G18a** | Lockfile integrity *(mirror)* | CI builds/tests/audits with `--locked` (Rust) and `pnpm install --frozen-lockfile`; a post-install `git diff --exit-code Cargo.lock pnpm-lock.yaml` so a drifted lockfile **FAILS** rather than auto-resolving a different graph than the audited/SBOM'd one (§3.8 pin-everything) | push | always; fail-closed |
| **G19** | Generated-artifact drift *(mirror)* | regenerate Tauri→TS bindings / CLI `--help` / asset manifest, then `git diff --exit-code`; + structural (parsed, not regex) non-empty sanity | push | by glob; fail-closed |
| **G20** | `plan-lint`/`spec-lint` full *(mirror)* | the G7 script, verbose, all checks | push | always; fail-closed |
| **G21** | Deferral gate full *(mirror)* | the G8 scan vs `origin/main` | push | new-marker; fail-open w/o base |
| **G22** | Schema/membership parity *(mirror)* | "every supported format ∈ README matrix ∧ has a fixture ∧ has a round-trip test"; locale-file key parity (if i18n) | push | by glob; fail-closed |
| **G23** | "every X has a Y" completeness *(mirror)* | e.g. every `convert_*` command has a test (via `git ls-files`) — caveat: tracking-aware, stage partner file together | push | by glob; fail-closed |
| **G24** | Gate-script self-tests *(mirror)* | run the custom-gate unit tests | push | when a gate script changed; fail-closed |
| **G18b** | First-party crate-trust audit *(mirror)* | **`cargo-vet`** — records per-crate trust audits and **fails when an unvetted/changed crate enters the tree** (closes the gap G17 leaves: a *new* malicious/typosquatted crate has no advisory yet, so `cargo audit` can't see it); offline-friendly (audits are committed) | push | always; fail-closed |
| **G53** | Core-crate forbidden-dependency *(mirror)* | assert the core crate's Cargo dependency closure does **NOT** contain the image-worker-only C libs (`libvips`/`libheif`/`librsvg`/`libimagequant`) — the build-time analogue of the §3.6 "LGPL must not link into the MIT core" assertion (T6); a careless refactor that pulls a copyleft C lib into the core fails here | push | always; fail-closed |
| **G54** | Hooks-installed assertion *(mirror)* | a `post-checkout`/`post-merge` hook (or a Lane-A check that the Lefthook config hash matches a fresh install) asserting **`lefthook install`** ran — a clone that skipped it has no local L1–L3 protection and (single-Build-Loop model) **no PR gate to catch it**; "`lefthook install` is mandatory after clone" is documented (build-loop.md) | push | always; fail-open only if Lefthook absent by design (CI never absent) |

**Fastpath / skip (L2 only).** Expensive hooks (G15/G16/G17/G18 + heavy lint) are
skipped **only** when provably irrelevant, via detectors that each default to
*run* on ambiguity:
- **Docs-only push** — the hard safety guard is the **RANGE diff over ALL unpushed
  commits** (`git diff --name-only @{u} HEAD`, fallback chain below), **not** the
  HEAD-commit subject: if every changed path across the unpushed range is markdown
  ⇒ no Rust/TS/lockfile to scan ⇒ safe to skip the byte-scanning gates. A code
  change in *any* unpushed commit forces the full gate even if HEAD is docs-only.
- **Check-off fastpath** — `chore(todo): … abgehakt`-style subject **AND** a
  markdown-only diff.
- Detector fallback chain: `@{u}` → `origin/<branch>` → `origin/main` →
  `origin/HEAD`; **no base / 0 unpushed commits ⇒ run the full gate.** Skipping is
  opt-in; anything ambiguous runs everything.
- Cheap structural gates (G13/G20/G21/...) and glob-gated gates always run when
  their glob matches — they have nothing expensive to skip.

## 5. L4 — CI (GitHub Actions, post-push) & L5 — Release (`v*` tag)

### L4 — CI (clean checkout; mirrors L1–L2 + the heavy gates)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G25** | All L1–L2 gates re-run on clean checkout | the same hooks/scripts in CI | red `main` |
| **G26** | Full fuzz pass (engine-side T1 = corpus fault-injection) | the §6.4.2 corpus/no-harm fault-injection **through the §2.12 isolation boundary** (truncated/0-byte/fuzzed-header/decompression-bomb inputs → one plain message, no crash, batch continues). *(`cargo-fuzz`/libFuzzer is in-process Rust and CANNOT reach the isolated C/C++ engines — the in-core fuzzable surface is **G48**; this row is the engine-side T1 control.)* | red `main` |
| **G27** | Coverage — per-domain floors | **`cargo-llvm-cov`** (Rust, LLVM branch) **and** **vitest v8** (TS) — **separate** floors, fail if **EITHER** is below its floor (never averaged); ratchet **50 % → 70 %** stored in a tracked file (can only increase; a commit that lowers it fails; raises are deliberate committed config changes — no auto-increment). The G48 saved-crash-corpus replay does **not** count toward the Rust floor | red `main` |
| **G28** | Coverage — diff gate | **≥ 80 %** on changed lines (change-only) so new code can't dilute the floor | red `main` |
| **G29** | SAST / static security | **unsafe-policy gate (primary):** `#![forbid(unsafe_code)]` at the core-crate root with a small **allow-listed FFI module** (the §2.1/§2.3 OS primitives `renameat2`/`MoveFileExW`/`GetFileInformationByHandle`, the §0.9 Job-Object kill) — the gate is "**no new `unsafe` block outside the allow-listed FFI module**" (clippy `unsafe_code` lint / diff scan requiring a `// SAFETY:` comment). **Semgrep** packs `p/rust` + `p/typescript` + `p/security-audit` + a project-local ruleset (`tauri::command` handlers taking `PathBuf` from the WebView; `process::Command` constructed outside `crate::isolation`) — pin the rules/image digest + vendor for offline reproducibility. **`cargo-geiger` is INFORMATIONAL only** (a census, not an enforcer — version-fragile; never a required green check) | red `main` (Semgrep + unsafe-policy block; geiger informational) |
| **G30** | Cross-platform build matrix | native build on Windows / macOS / Linux runners (no cross-compile); macOS universal `lipo` | red `main` |
| **G31** | Per-pair corpus + reliability | real-file round-trips per `(source→target)` pair per platform; output validated (codec/container/header), not "file exists"; reliability threshold. Also hosts the fs-safety/membership/freeze/redaction/temp-mode/resource-budget integration assertions (T2a/T2b/T4/T9b/T10) when their input phase exists | red `main` |
| **G32** | Round-trip invariant | A→B→A byte-stable (lossless) / within tolerance (lossy) as a CI gate | red `main` |
| **G33a** | a11y — ARIA/role/focus (per-PR) | **`vitest-axe`** (axe-core under jsdom) over the rendered React tree: ARIA-role/state validity + focus-order / roving-tabindex sanity. Lane-A per-PR (jsdom cannot compute contrast) | red `main` |
| **G48** | In-core detector fuzz | **`cargo-fuzz`** (libFuzzer) target over `crate::detect`/sniff on a hostile corpus (malformed ZIP central-directory, OLE2/CFB, gzip/svgz, XML) asserting: no panic/abort, the §1.2 decompression-ratio cap (≤100×) + the `MAX_SVGZ_SNIFF` (≤64 KiB) bound actually fire, and the XML reader has **DTD/external-entity resolution disabled by construction** (a `quick-xml`/`roxmltree` reader with entity resolution off — defeats XXE / billion-laughs in the `xl/workbook.xml` / ODS `content.xml` peek). Constrained to where libFuzzer is reliable (**Linux + macOS, nightly toolchain**); the L2 leg is the deterministic G16 replay, never an instrumented Windows build | red `main` |
| **G49** | *(see L1 — `actionlint`, mirrored)* | mirrored in CI on a clean checkout | red `main` |
| **G50** | Workflow security lint | **`zizmor`** (Rust GH-Actions static analyzer) — flags unpinned actions (mutable tags vs full commit SHA), dangerous `pull_request_target`, template-injection via untrusted `${{ github.event.* }}` in `run:` steps, and excessive `GITHUB_TOKEN` scope | red `main` |

**Per-push adversarial-egress pull-forward.** On runners that support the §6.7.3
enforcement path, the §6.4.2 adversarial-egress corpus (HLS `m3u8`, DASH `mpd`,
`concat` script, external-`href` SVG, remote-`<img>` pandoc, `WEBSERVICE()` xlsx)
runs in the per-push **L4** integration leg (under G42's egress-deny window) so a
**T9b** egress regression introduced in P6/P7 is caught on the push that introduced
it; **G42** is the final release confirmation. (The macOS WebView leg degrades to
the §6.6 walkthrough — see G42/G33b.)

> **G34 is intentionally vacated.** A screenshot/visual-regression gate has **no
> §-home in the spec** (§6.4.6 is the WebDriver flow; §6.4.6a is axe) — a
> release-blocking gate with no spec home is not added. If visual-regression is
> wanted it must be added to the spec's §6.4.6 family **first** (flagged as an idea,
> §7); the id `G34` stays reserved/unused so existing references do not renumber.

### L5 — Release (tag-triggered; release-blocking)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G33b** | a11y — WCAG-AA contrast (release-tier) | **`@axe-core/webdriverio`** against the live WebView (`tauri-driver`) — WCAG 2.1 AA `color-contrast`, **both** themes, on the **Linux + Windows** legs (jsdom cannot compute contrast). **macOS is the acknowledged automated gap** (`tauri-driver` has no WKWebView driver, §6.4.6) → satisfied by the §6.6 human walkthrough's readable-contrast check, recorded in `docs/usability-floor.md` | release |
| **G35** | SBOM generation + completeness | **generation:** `cargo cyclonedx` (Rust) + **`@cyclonedx/cyclonedx-npm`** (the official package; `--spec-version 1.5`) merged via §3.7.2 `cargo xtask sbom` with `engines.lock`. **completeness (MANDATORY, not optional):** **`Syft`** scans the staged bundle so every shipped executable/lib/font ∈ `engines.lock`+declared sub-components, no `UNKNOWN`/`NOASSERTION` (except the §6.3.3 `LicenseRef` carve-out); **backed by** a deterministic stage-tree file-manifest diffed against `engines.lock` (Syft can miss libs statically compiled INTO FFmpeg/LibreOffice) so an unexpected `.so`/`.dll`/`.dylib` **hard-fails** | release |
| **G36** | License hard-fail | SBOM scanned for forbidden families (GPL/AGPL in MIT core / static Rust binary) → exit 1 | release |
| **G37** | Engine checksum / integrity build gate | pinned-version + SHA-256 verify of every bundled engine **against the change-reviewed in-repo `engines.lock`** *before* staging **AND re-verified on cache-restore** (an Actions cache is not integrity-protected — on mismatch, delete + refetch from the pinned upstream URL); build-time in-bundle hash manifest generated | release |
| **G38** | Per-engine build assertions | FFmpeg `-protocols`/`-demuxers` curated + required-codec lock + `concat -safe 1`; librsvg no-base-URL API; libvips no-copyleft-PDF-loader (no `pdfload`/`poppler`/`mupdf` loader); LGPL shared-object-or-fail (carve-out i); libimagequant BSD-2-Clause leg-text + lockfile pin; the §6.1.3/§0.10 `tauri-plugin-store`-cannot-escape-`config_dir` assertion (T2c) | release |
| **G38b** | Copyleft corresponding-source bundle present | the §6.1.3 carve-out ii/iii **bundle-presence** assertion: for the static image-worker (LGPL §6) ship its complete corresponding source + LGPL object files / relink recipe, **and** because it links GPL x265 ship the **x265 GPL §3 complete corresponding source + written offer** — the stage step **fails the build if the source bundle is missing**. Maps to the §5 **T6** row | release |
| **G39** | Checksums + minisign | per-file SHA-256 + minisign detached signature **over `SHA256SUMS`** (pubkey at `docs/minisign.pub`, private key = `MINISIGN_SECRET_KEY` CI secret, rotation policy §6.2.3). **`minisign -Sm SHA256SUMS`** is the actual step — **provision the key AND wire the step**. This is the **only** signing in scope (§6.2.3) — **not** binary code-signing/notarization (SSOT *Out of Scope*) | release |
| **G55** | Auditable Rust binary | build the shipped Rust core with **`cargo auditable build --release`** so the dependency list is embedded in the binary (~4 KB, zero CI cost) — a portable, no-auto-update artifact can be audited **from the binary alone** long after a CVE drops (`cargo audit bin` / grype consume it); a strong fit for an offline "audit it yourself" MIT product | release |
| **G41** | Artifact size budget | per-platform compressed artifact ≤ budget (≤400 MB target) — measured after bundle | release |
| **G42** | Offline-egress: active-deny + observe-the-attempt | mirror spec §6.7.3 — an **OS-level egress-DENY window** (the enforcement) **plus** a packet monitor (the proof) on each platform: **Linux** netns/nftables drop with `iptables -j LOG`/`NFLOG` + an `strace -e trace=network` `connect()`/`getaddrinfo` leg (a *blocked-but-attempted* connection is caught, so a silent DROP can't make "zero packets" prove nothing); **macOS** `pf` `block log` → `pflog0` read by `tcpdump`; **Windows** outbound firewall block **with** dropped-packet logging / ETW. The §7.2.3 startup engine smoke probes run **inside the same window**; engine spawns assert `.env_clear()` (no inherited `http_proxy`/`HTTPS_PROXY`/`*_PROXY`/`LD_PRELOAD`/`DYLD_*`) as a unit-testable invariant. Zero egress except the user-initiated releases-page. **macOS WKWebView leg is driver-gapped (§6.7.3)** — core/engine egress is asserted there, the WebView's via §6.6 + static inspection | release |
| **G43** | No-system-pollution audit | syscall/fs monitor during a conversion → no registry/LaunchAgent/daemon/file-assoc writes; writes only to config+log+chosen-output+scratch | release |
| **G44** | Governance completeness | every required governance doc present + non-stub; download/trust page complete (verify recipe, WebView2/FUSE/Sequoia notes) | release |
| **G45** | Name/trademark clearance record | `docs/name-clearance.md` present, dated for the release line, verdict = clear; dormant rename-propagation + old-name grep gate | release |
| **G46** | Startup integrity (runtime, also a release acceptance) | engine presence + integrity verification; missing/corrupt engine → app-fault, not crash | release acceptance |

## 6. The `plan-lint` / `spec-lint` doc-consistency gate (G7/G20)

A single **stdlib-only** script (no third-party deps → runs anywhere instantly),
treating our canonical docs as machine-checkable truth. Exit codes: `0` none,
`1` ≥1 finding (**any** severity), `2` target missing. CLI: `--check <ids>`,
`--json`, `--quiet`, `--max-per-check N`. Three call sites: L1 (`--quiet` on glob),
L2 (full), L4.

Invariant checks (initial set; expanded during P0 review):
1. **Membership / matrix parity** — every format named in prose ∈ the README
   support matrix, and every matrix row has a code registry entry + a fixture + a
   round-trip test (ties G22).
2. **Cross-reference validity** — every `§X.Y` / internal anchor resolves.
3. **Heading hierarchy** — no skipped levels.
4. **Numbering gap-freeness** — sub-sections run min→max, no gaps.
5. **Gate-catalogue integrity** — every gate named in security-concept.md exists
   here as a `Gnn` row, and vice-versa (membership, not phrasing).
6. **No forbidden tokens** — banned stamps/strikethrough/stale-dates in doc bodies.
7. **Generated-file structural sanity** — when validating a generated file, parse
   it (not regex) and assert non-empty/well-formed.
8. **§0.11 ↔ §5 threat-map parity (bidirectional)** — every spec §0.11 class
   (`T1, T2, T2a, T2b, T2c, T3, T4, T5, T6, T7, T8, T9a, T9b, T10`) has exactly one
   row in security-concept.md §5, and every §5 threat row cites a `Gnn` that exists
   in this catalogue. Fails the build if a class loses its row or a row loses its
   gate (so the mapping can never silently drift).
9. **Inventory parity (membership checks)** — every IPC command `C1..C13` named in
   prose ∈ §0.4.1; every engine id in prose ∈ §3.1; the fixed-set enums
   (`FormatId`, `EngineProgram`, `PatentDisposition`, the error taxonomy, the
   lossy-catalog) are internally consistent across the files that reference them.
   (Brings the linter toward the RMA 10-check depth.)

## 7. Reconciled during P0 review r1

- **Concrete tools picked (closed).** secrets-scanner = **`gitleaks`** (G2);
  CSP/capability-lint = **G47** (`jq`/`serde_json` structural); SAST = `#![forbid(unsafe_code)]`
  + Semgrep packs (`cargo-geiger` informational only, G29); in-core fuzz harness =
  **G48** (`cargo-fuzz` over `crate::detect`, Linux+macOS nightly); per-OS observability =
  G42 (netns/nftables+strace · `pf` block-log+tcpdump · Windows firewall-block+ETW);
  CI hardening = `actionlint` (G49) + `zizmor` (G50) + lockfile integrity (G18a).
- **Every §0.11 class has a verifying gate (closed).** Enforced mechanically by
  plan-lint check 8 (§6); the new gates G47/G48/G38b/G17b close the previously
  runtime-only classes T2/T2a/T2c, the in-core T1 path, T6's source-bundle, and the
  T3 CVE-awareness signal.
- **Living-doc/spec-sync.** Gates added here that the spec only named as `[DEFER]`
  (CSP-lint) or did not name (in-core detector fuzz, the SAST layer, `actionlint`/
  `zizmor`/lockfile-integrity) are reconciled into the spec **in the same change**
  per the SSOT > spec > docs conflict order.

**Still owner-decidable (ratchet plan, not blocking the P0 *design*):**

- [ ] Which L4 gates are *required checks* vs informational on day one, so a
  half-built P1 isn't wedged (G17b/`cargo-geiger` are informational by design).
