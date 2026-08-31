//! `crate::engines::lock` — the §3.7.2 `engines.lock` build-manifest SCHEMA (§0.7 physical tree:
//! `engines/`, tier 2): the data contract every staged artifact declares itself under, plus the
//! validator that rejects a malformed manifest.
//!
//! `engines.lock` (in `src-tauri/`, the single canonical name — there is no `engines.toml`) is the
//! **authoritative input** to the release plane, "not hand-curated prose, so it can't drift from what
//! actually ships" (§3.7.2).
//!
//! ## The row law `[DECIDED — owner adjudication 2026-08-31, §3.4.4a]`
//!
//! A row is keyed by **(artifact, target-triple)**. The TRIPLE is the key, not the OS: §3.4.5 gives
//! macOS two of them (`aarch64-apple-darwin` + `x86_64-apple-darwin`) and the universal artifact is
//! `lipo`-merged from both, so an OS-keyed row could not name what actually ships. **The OS level is
//! the DERIVED view** — §3.4.4a's parse→map flow resolves the running target's row and projects it
//! onto the §3.2.2 `Platform` that `PatentDisposition` carries.
//!
//! From that key follow four properties this module enforces:
//! * **Every field is platform-pure and scalar.** The ONLY plural field is [`EngineRow::triples`].
//! * **A byte-identical, platform-invariant artifact is ONE row over several triples** — a bundled
//!   font, the ImageMagick `policy.xml`, the LibreOffice `registrymodifications.xcu` — rather than N
//!   copies that could drift apart under a hand edit.
//! * **(artifact, triple) ↦ exactly one row, and one row = one `sha256`.** The moment any field forks
//!   across triples, the row splits.
//! * **§3.7.2 item 4's sub-component rows follow the same law**, with the hash anchoring the pinned
//!   SOURCE rather than a staged file (a statically-absorbed lib like `libimagequant` ships no
//!   standalone artifact) — hence [`RowKind`], which says what a row's `sha256` is OF.
//!
//! ## Consumers
//! Each reads a different slice, which is why the contract lives in one place:
//! * **§3.4.4a / P4.40** — the startup sequence parses the manifest ONCE and maps the running target's
//!   codec row [`EngineRow::available`] into a `PatentDisposition`, built BEFORE any
//!   `Engine::capabilities(platform, patents)` call. That flag is the SOURCE of the posture, not a
//!   second truth.
//! * **§6.3.1 / P4.56.2** — `cargo xtask sbom` merges these rows into the CycloneDX component set,
//!   which is why [`EngineRow::purl`] is mandatory: G17b/G37 need a named key, not an implied one.
//! * **§3.8 / P4.56.3** — the engine-source allow-list gate reads [`EngineRow::upstream_url`] and
//!   [`EngineRow::corroboration_url`] and fails on an off-allow-list or same-origin pair.
//! * **§6.1.3 / P4.28** — the asset fetcher keys the cache by `<engine>-<version>-<triple>`, which is
//!   exactly this row key.
//!
//! ## Scope (P4.56.1) — the CONTRACT, not the file
//! `engines.lock` itself is L(-1)-caged, so the committed manifest is an owner-acked act and the
//! per-engine rows land with their staging boxes (P5–P7). What lives here is the shape, the
//! validator and their tests. Deliberately NOT here: the TOML READ of the real file. The types derive
//! [`serde::Deserialize`], so any serde format can drive them; the runtime read is **P4.40**'s, the box
//! whose §3.4.4a flow needs it, and promoting a TOML parser into the shipped MIT-core closure belongs
//! with that production consumer — the same dep-promotion discipline `serde_json` (P2.85) and
//! `tempfile` (P3.4) followed. [Build-Session-Entscheidung: P4.56.1]
//!
//! Validation collects EVERY violation rather than failing on the first: a manifest is hand-edited
//! under owner-ack, so one round-trip should surface every problem in it.

use serde::Deserialize;

/// The §3.4.5 v1 target triples a row may be keyed by. `universal-apple-darwin` is deliberately
/// absent: it is a `lipo` BUILD OUTPUT merged from the two per-arch macOS triples, never a row key —
/// a row names bytes that were actually staged for one target.
///
/// This is a SECOND copy of the §3.4.5 set (`scripts/stage-engines` holds the staging-side one) with
/// no mechanical drift catcher: when a `[DEFER]`red triple lands — Windows-aarch64, Linux-musl —
/// nothing fails until a row uses it and this list rejects it. A cross-check would have to live in a
/// `scripts/check-*` gate, i.e. an L(-1) act, which is disproportionate for a list whose additions are
/// themselves owner-acked §3.4.5 spec edits; the box that widens §3.4.5 widens both copies.
/// [Build-Session-Entscheidung: P4.56.1 — raised by the dual review]
const V1_TARGET_TRIPLES: [&str; 4] = [
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
];

/// The parsed `engines.lock` manifest — an array of per-(artifact, triple) rows (`[[engine]]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnginesLock {
    /// The rows. Empty is VALID: the manifest exists as a container from P4.56.1 and fills as P5–P7
    /// stage each engine (the same skip-with-notice posture the staging and allow-list gates hold).
    #[serde(default)]
    pub engine: Vec<EngineRow>,
}

/// What a row's `sha256` is OF — the §3.7.2 item-4 distinction the row law preserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RowKind {
    /// A file that ships in the bundle: the hash is of the STAGED bytes, and G37 verifies the staged
    /// file against it before staging and again on cache-restore.
    StagedArtifact,
    /// A component absorbed by a link and shipping no standalone file (§3.7.2 item 4 — e.g. the
    /// statically vendored `libimagequant`): the hash ANCHORS the pinned source, since there are no
    /// staged bytes to verify. G35a derives such components from the link and fails if one has no row.
    SubComponent,
}

/// How the artifact relates to the MIT core — the field the §3.6.1 copyleft-isolation argument reads,
/// and the one CLAUDE §3's "MIT core clean; copyleft isolated" guardrail keys on.
///
/// §3.1 names THREE relationships, not two: the GPL set is "always **invoked** or
/// **dynamically-plugin-loaded**, never statically **linked** into the MIT core". A two-variant enum
/// could not express the x265 libheif plugin — the very component §3.4.4a's `available` flag governs —
/// and would have forced P5.9 to mis-declare an obligation. [Build-Session-Entscheidung: P4.56.1 —
/// raised by the dual review, which found the fixture modelling x265 as `invoked`.]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Linkage {
    /// Linked into a first-party binary at build time (the libvips/libheif/librsvg image stack inside
    /// `convertia-imgworker`, §3.5.5 — aggregation because the worker is its own binary, §3.6.1 ii).
    Linked,
    /// Spawned as its own process (FFmpeg / LibreOffice / poppler / pandoc, §3.6.1 aggregation).
    Invoked,
    /// Loaded at RUNTIME into an already-running first-party process as a plugin (§3.1 row 1a: x265
    /// ships as a dynamically-loaded libheif encoder plugin, never statically linked). Distinct from
    /// both neighbours because the obligation differs: §3.6.1's x265 row makes the image-worker a GPL
    /// **combined work** while it is loaded, which neither an invoked sidecar nor the LGPL static link
    /// implies.
    PluginLoaded,
}

/// How the artifact was obtained (§3.8 `[DECIDED]`, per engine PER TRIPLE — which is why it is a
/// scalar on a triple-keyed row). The mode decides what the recorded SHA-256 actually PROVES: for
/// `from-source` it is a build-output stability check and the provenance anchor moves to
/// [`FromSourceAnchor`]; for `prebuilt` the hash is corroborated against the upstream's own published
/// checksum/signature (or, where the upstream publishes neither, ≥ 2 independent mirrors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Acquisition {
    /// A third-party prebuilt binary, corroborated per §3.8's prebuilt rule.
    Prebuilt,
    /// Compiled by our own CI from a verified source release (§3.8's preferred mode).
    FromSource,
}

/// The NAMED tool a from-source signature was verified with (G37: verified with `gpg --verify` /
/// `sq verify` at pin-establishment). Recorded because G37 requires the VERIFICATION, not just the
/// resulting hash — a bare hash of a signed tarball is the xz/liblzma class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationTool {
    /// `gpg --verify`.
    Gpg,
    /// `sq verify` (Sequoia).
    Sq,
}

/// The §3.8 / G37 from-source provenance anchor set — REQUIRED on a `from-source` row and forbidden on
/// a `prebuilt` one, because the two modes have different ground truths.
///
/// G37 is explicit that the binary hash is "a build-output stability check, NOT a provenance anchor"
/// for this mode: the anchor is the signed source plus a digest-pinned toolchain, and the VCS
/// tag/commit is recorded alongside because the xz/liblzma backdoor rode a validly-signed tarball
/// whose generated files differed from the upstream git tree ([`EngineRow::source_ref`] carries it).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromSourceAnchor {
    /// SHA-256 of the signed SOURCE tarball — distinct from the row's artifact/anchor hash.
    pub tarball_sha256: String,
    /// The upstream signing key pinned in-repo. A change here is the same hard escalation as a SHA
    /// edit (§3.8), which is why it is recorded rather than implied.
    pub signing_key_fingerprint: String,
    /// Which named tool performed the verification.
    pub verified_with: VerificationTool,
    /// The digest-pinned build toolchain / base image (§6.1.3) — without it the toolchain is the
    /// unverified input.
    pub toolchain_digest: String,
}

/// One row: one artifact on one target triple (or, for a byte-identical platform-invariant artifact,
/// on several — see the module's row law).
///
/// `deny_unknown_fields` is load-bearing, not tidiness: a typo'd key in a hand-edited, owner-acked
/// manifest would otherwise be silently DROPPED — a mistyped `sha256` would read as "no hash declared"
/// rather than as an error, which is the exact class this manifest exists to make impossible.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineRow {
    /// The artifact's identity — the staged file or component this row is ABOUT. Not the project name:
    /// `libmp3lame.so` and the FFmpeg exe beside it are two artifacts. Unique per triple, not globally.
    pub id: String,
    /// The §3.8 pinned exact version.
    pub version: String,
    /// The §3.8 **source ref** the pin is anchored to — the upstream VCS tag or commit. §3.8 pins "an
    /// exact version + source ref", and G37 requires BOTH the signed-tarball SHA and the upstream
    /// tag/commit to be recorded here, because a tarball-only provenance is the xz/liblzma class.
    /// P4.54 reads it to assert the `libimagequant` pin is exactly the `lovell` v2.4.x-fork commit.
    pub source_ref: String,
    /// The §3.4.5 target triples this exact artifact ships on. The ONLY plural field: several entries
    /// mean the bytes are byte-identical across those targets (a font, a config file), so one row
    /// covers them rather than N copies that could drift.
    pub triples: Vec<String>,
    /// Whether the `sha256` below is of staged bytes or anchors a pinned source (§3.7.2 item 4).
    pub kind: RowKind,
    /// The upstream URL the artifact (or its source) came from. §3.8's allow-list gate (P4.56.3)
    /// constrains WHICH origins are permitted; the schema only requires a usable URL.
    pub upstream_url: String,
    /// The SPDX licence id / expression. §6.3.3 + G36 validate it with a real SPDX parser; the schema
    /// only requires it to be declared, so a missing licence can never reach the NOTICE assembly.
    pub licence: String,
    /// The §3.6.1 copyleft-isolation class — linked, invoked, or runtime plugin-loaded (§3.1).
    pub linkage: Linkage,
    /// §3.8 acquisition mode — what the SHA-256 below proves.
    pub acquisition: Acquisition,
    /// The MANDATORY package URL, `pkg:generic/<name>@<version>` minimum (§3.7.2 / G35): the named key
    /// G17b's CVE matching and the CycloneDX component both need.
    pub purl: String,
    /// A CPE where one exists (§3.7.2) — optional by contract, because many bundled components have no
    /// registered CPE and a fabricated one would match nothing while looking like coverage. (G17b
    /// tightens this to MANDATORY for a named high-CVE subset; that escalation is G17b's to enforce.)
    #[serde(default)]
    pub cpe: Option<String>,
    /// The per-row SHA-256, lowercase hex — of the staged bytes or of the pinned source, per
    /// [`EngineRow::kind`].
    pub sha256: String,
    /// The §3.7.2 `[DECIDED]` pin-establishment provenance: WHERE the hash was corroborated. Recording
    /// a hash of an unverified first download only launders provenance, so the corroboration source is
    /// part of the pin, not an optional note.
    pub corroboration_url: String,
    /// The §3.4.4a availability flag — a SCALAR bool, present only on an encumbered codec's row.
    /// Flipping it is a config change (edit + rebuild), never a code change.
    #[serde(default)]
    pub available: Option<bool>,
    /// The §3.8 / G37 from-source anchor set — required iff `acquisition = "from-source"`.
    #[serde(default)]
    pub from_source: Option<FromSourceAnchor>,
}

/// One way a manifest violates the §3.7.2 contract. Every variant carries the row INDEX and the row's
/// `id`, so a failure names the offending row rather than the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockViolation {
    /// A required string field is present but empty (serde catches an ABSENT field; only a
    /// present-but-blank value reaches here).
    EmptyField {
        /// 0-based index of the row in the manifest.
        row: usize,
        /// The row's declared `id` (empty when it is the `id` itself that is blank).
        id: String,
        /// The offending field name.
        field: &'static str,
    },
    /// A row declares no triple, so it names no target — the row key would be incomplete.
    NoTriples {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
    },
    /// A triple is not in the §3.4.5 v1 set (`universal-apple-darwin` included: it is a build output,
    /// never a row key).
    UnknownTriple {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The offending value.
        triple: String,
    },
    /// The (artifact, triple) key is not unique — two rows claim the same artifact on the same target,
    /// so "one row = one sha256" no longer determines which bytes are expected.
    DuplicateKey {
        /// 0-based index of the SECOND of the two colliding rows.
        row: usize,
        /// The repeated id.
        id: String,
        /// The triple both rows claim.
        triple: String,
    },
    /// Two STAGED-artifact rows carry the same hash. Byte-identical staged artifacts must be ONE row
    /// over several triples (the row law), so a repeat means a merge that was not done — and two rows
    /// that could drift. Sub-component rows are exempt: their hash anchors a triple-invariant SOURCE,
    /// so a component whose per-target compile differs MUST split into rows that share that anchor.
    DuplicateSha256 {
        /// 0-based index of the SECOND of the two colliding rows.
        row: usize,
        /// The row's id.
        id: String,
        /// The repeated hash.
        sha256: String,
    },
    /// The `purl` is not the §3.7.2 `pkg:generic/<name>@<version>` minimum.
    MalformedPurl {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The offending value.
        purl: String,
    },
    /// The `purl`'s version does not match the row's own `version` — a mismatched key matches nothing
    /// in G17b's CVE lookup, and a green-but-empty report reads as "no known CVEs".
    PurlVersionMismatch {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The version the purl carries.
        purl_version: String,
    },
    /// A SHA-256 field is not exactly 64 lowercase hex digits.
    MalformedSha256 {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The offending field name (`sha256` or the anchor's `tarball_sha256`).
        field: &'static str,
        /// The offending value.
        sha256: String,
    },
    /// A `cpe` is present but is not a 13-component CPE 2.3 formatted string.
    MalformedCpe {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The offending value.
        cpe: String,
    },
    /// A URL field carries no scheme, so no origin can be derived from it (P4.56.3's allow-list gate
    /// compares ORIGINS; a scheme-less string has none).
    SchemelessUrl {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The offending field name.
        field: &'static str,
        /// The offending value.
        url: String,
    },
    /// Two rows share an `id` but disagree on a field that identifies the COMPONENT rather than the
    /// per-target artifact. The row law splits a component across triples precisely so per-target
    /// facts can differ — but `version` / `source_ref` / `licence` / `purl` describe the pinned
    /// component itself, so a disagreement is the hand-edit drift the law exists to prevent, one hop
    /// over: not two values inside one row's `triples`, but two sibling rows of the same id.
    ///
    /// Deliberately NOT in this set: `acquisition` and the two URLs, because §3.8 decides prebuilt-vs
    /// -from-source **per engine PER PLATFORM**, so a component legitimately prebuilt on one triple and
    /// compiled on another has different modes and different download/corroboration URLs. `cpe` is out
    /// too — it is optional by contract, so a present/absent pair is not evidence of drift. And `kind`
    /// is out DELIBERATELY, not by oversight: §6.1.3's carve-out iii lets a component be a separately
    /// staged object on one triple and absorbed by a static link on another, so it genuinely forks —
    /// which matters, because `kind` is what scopes the duplicate-hash rule. `from_source.verified_with`
    /// is out for its own reason: two siblings recording the same tarball hash and the same key under
    /// different tools mean the signature was checked twice at pin-establishment, which is not drift.
    IdFieldMismatch {
        /// 0-based index of the row that disagrees with the first row carrying this id.
        row: usize,
        /// The shared id.
        id: String,
        /// The component-identifying field that differs.
        field: &'static str,
    },
    /// The from-source anchor set is missing on a `from-source` row, or present on a `prebuilt` one.
    AnchorModeMismatch {
        /// 0-based row index.
        row: usize,
        /// The row's id.
        id: String,
        /// The row's declared acquisition mode.
        acquisition: Acquisition,
    },
}

/// The §3.7.2 minimum purl form: `pkg:generic/<non-empty name>@<non-empty version>`. Returns the
/// version so the caller can cross-check it against the row's own.
#[must_use]
fn purl_version(purl: &str) -> Option<&str> {
    let rest = purl.strip_prefix("pkg:generic/")?;
    // `rsplit_once` so a name containing `@` keeps the LAST `@` as the separator.
    let (name, version) = rest.rsplit_once('@')?;
    (!name.is_empty() && !version.is_empty()).then_some(version)
}

/// A SHA-256 as it is written in the manifest: exactly 64 LOWERCASE hex digits. Case is fixed on
/// purpose — a mixed-case hash would compare unequal to a tool's lowercase output and read as a tamper
/// hit rather than a formatting slip.
#[must_use]
fn sha256_is_well_formed(sha256: &str) -> bool {
    sha256.len() == 64
        && sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A CPE 2.3 FORMATTED STRING: `cpe:2.3:` plus eleven further components, thirteen in all, whose
/// `part` is one of `a`/`o`/`h`. A prefix-only check would accept `cpe:2.3:junk`, which matches nothing
/// in a CVE lookup while looking like coverage — the same failure mode a wrong purl has.
#[must_use]
fn cpe_is_well_formed(cpe: &str) -> bool {
    let parts: Vec<&str> = cpe.split(':').collect();
    parts.len() == 13
        && parts[0] == "cpe"
        && parts[1] == "2.3"
        && matches!(parts[2], "a" | "o" | "h")
}

/// A URL from which an ORIGIN can be derived — the property P4.56.3's allow-list comparison needs.
#[must_use]
fn url_has_scheme(url: &str) -> bool {
    match url.split_once("://") {
        Some((scheme, rest)) => !scheme.is_empty() && !rest.is_empty(),
        None => false,
    }
}

/// The wire token of a [`Linkage`], so sibling rows can be compared field-wise like the string fields.
#[must_use]
fn linkage_token(linkage: Linkage) -> String {
    match linkage {
        Linkage::Linked => "linked".to_owned(),
        Linkage::Invoked => "invoked".to_owned(),
        Linkage::PluginLoaded => "plugin-loaded".to_owned(),
    }
}

impl EnginesLock {
    /// Check the manifest against the §3.7.2 contract + the §3.4.4a row law, collecting EVERY
    /// violation.
    ///
    /// # Errors
    /// The full list of [`LockViolation`]s in row order. An empty manifest is valid (the container
    /// exists before the rows do).
    pub fn validate(&self) -> Result<(), Vec<LockViolation>> {
        let mut problems = Vec::new();
        let mut keys: Vec<(String, String)> = Vec::new();
        let mut hashes: Vec<&str> = Vec::new();
        // The FIRST row seen for each id, to compare its sibling rows' component-identifying fields
        // against. Comparing against the first (rather than pairwise) keeps the report to one
        // violation per drifting field per row instead of a quadratic blow-up.
        let mut first_by_id: Vec<(&str, &EngineRow)> = Vec::new();
        // The anchor baseline is tracked SEPARATELY — the first sibling that actually HAS an anchor,
        // not row 0 of the id. §3.8 makes mixed modes legitimate (prebuilt on one triple, compiled on
        // another), so keying the anchor comparison off row 0 would silently skip it whenever row 0
        // happens to be the prebuilt one, letting the from-source siblings drift on the very field the
        // comparison exists to pin. [Build-Session-Entscheidung: P4.56.1 — raised by the dual review]
        let mut first_anchor_by_id: Vec<(&str, &FromSourceAnchor)> = Vec::new();

        for (row, entry) in self.engine.iter().enumerate() {
            let id = entry.id.trim().to_owned();
            for (field, value) in [
                ("id", &entry.id),
                ("version", &entry.version),
                ("source_ref", &entry.source_ref),
                ("upstream_url", &entry.upstream_url),
                ("licence", &entry.licence),
                ("purl", &entry.purl),
                ("sha256", &entry.sha256),
                ("corroboration_url", &entry.corroboration_url),
            ] {
                if value.trim().is_empty() {
                    problems.push(LockViolation::EmptyField {
                        row,
                        id: id.clone(),
                        field,
                    });
                }
            }

            if entry.triples.is_empty() {
                problems.push(LockViolation::NoTriples {
                    row,
                    id: id.clone(),
                });
            }
            for triple in &entry.triples {
                if !V1_TARGET_TRIPLES.contains(&triple.as_str()) {
                    problems.push(LockViolation::UnknownTriple {
                        row,
                        id: id.clone(),
                        triple: triple.clone(),
                    });
                    continue;
                }
                // The row key. `id` is compared TRIMMED so a stray space cannot mint a second identity
                // for the same artifact — the hand-edit class `deny_unknown_fields` also guards.
                let key = (id.clone(), triple.clone());
                if keys.contains(&key) {
                    problems.push(LockViolation::DuplicateKey {
                        row,
                        id: id.clone(),
                        triple: triple.clone(),
                    });
                } else {
                    keys.push(key);
                }
            }

            match purl_version(&entry.purl) {
                None => problems.push(LockViolation::MalformedPurl {
                    row,
                    id: id.clone(),
                    purl: entry.purl.clone(),
                }),
                Some(version) if version != entry.version.trim() => {
                    problems.push(LockViolation::PurlVersionMismatch {
                        row,
                        id: id.clone(),
                        purl_version: version.to_owned(),
                    });
                }
                Some(_) => {}
            }

            // The duplicate-hash rule is scoped to STAGED artifacts, and this is where `RowKind` earns
            // its keep. Its purpose is "an unmerged pair of staged artifacts that can drift" — but a
            // SUB-COMPONENT row's hash ANCHORS the pinned source, which is triple-INVARIANT, while its
            // `from_source.toolchain_digest` is triple-VARIANT (one per-target compile). The row law
            // therefore REQUIRES such a component to split per triple, and those rows legitimately
            // share one source anchor. Applying the rule to them would make the mandated shape
            // unwritable — the same reason `from_source.tarball_sha256` carries no duplicate rule.
            // [Build-Session-Entscheidung: P4.56.1 — raised by the dual review]
            if sha256_is_well_formed(&entry.sha256) {
                if entry.kind == RowKind::StagedArtifact && hashes.contains(&entry.sha256.as_str())
                {
                    problems.push(LockViolation::DuplicateSha256 {
                        row,
                        id: id.clone(),
                        sha256: entry.sha256.clone(),
                    });
                } else if entry.kind == RowKind::StagedArtifact {
                    hashes.push(entry.sha256.as_str());
                }
            } else {
                problems.push(LockViolation::MalformedSha256 {
                    row,
                    id: id.clone(),
                    field: "sha256",
                    sha256: entry.sha256.clone(),
                });
            }

            if let Some(cpe) = &entry.cpe {
                if !cpe_is_well_formed(cpe) {
                    problems.push(LockViolation::MalformedCpe {
                        row,
                        id: id.clone(),
                        cpe: cpe.clone(),
                    });
                }
            }
            for (field, url) in [
                ("upstream_url", &entry.upstream_url),
                ("corroboration_url", &entry.corroboration_url),
            ] {
                if !url.trim().is_empty() && !url_has_scheme(url) {
                    problems.push(LockViolation::SchemelessUrl {
                        row,
                        id: id.clone(),
                        field,
                        url: url.clone(),
                    });
                }
            }

            match first_by_id.iter().find(|(seen, _)| *seen == id) {
                // TRIMMED, matching the comparison above and `DuplicateKey`'s key. Pushing the raw id
                // would let a stray space mint a second identity that never matches its sibling,
                // silently disabling this whole check — the class the key comparison already closes.
                None => first_by_id.push((entry.id.trim(), entry)),
                Some((_, first)) => {
                    for (field, mine, theirs) in [
                        ("version", &entry.version, &first.version),
                        ("source_ref", &entry.source_ref, &first.source_ref),
                        ("licence", &entry.licence, &first.licence),
                        ("purl", &entry.purl, &first.purl),
                        // §3.6.1's copyleft-isolation class — the field CLAUDE §3's "MIT core clean;
                        // copyleft isolated" guardrail keys on, so higher-stakes than `licence`. It does
                        // NOT fork per triple: §6.1.3's carve-out iii (dynamic-beside-the-exe vs static
                        // FFmpeg) changes whether a lib is separately STAGED — i.e. `kind` and the row
                        // set — while libmp3lame stays `linked` into the FFmpeg binary either way.
                        (
                            "linkage",
                            &linkage_token(entry.linkage),
                            &linkage_token(first.linkage),
                        ),
                    ] {
                        if mine.trim() != theirs.trim() {
                            problems.push(LockViolation::IdFieldMismatch {
                                row,
                                id: id.clone(),
                                field,
                            });
                        }
                    }
                }
            }

            // Same `source_ref` entails the same signed tarball, hence the same tarball hash and the
            // same signing key — the xz-class provenance anchor. Baselined on the first sibling that
            // HAS an anchor (see `first_anchor_by_id`), so a prebuilt row appearing first cannot skip
            // the comparison. `verified_with` is deliberately NOT compared: two siblings recording the
            // same tarball hash and key under different tools mean the signature was checked twice,
            // which is not drift.
            if let Some(mine) = &entry.from_source {
                match first_anchor_by_id.iter().find(|(seen, _)| *seen == id) {
                    None => first_anchor_by_id.push((entry.id.trim(), mine)),
                    Some((_, theirs)) => {
                        for (field, a, b) in [
                            (
                                "from_source.tarball_sha256",
                                &mine.tarball_sha256,
                                &theirs.tarball_sha256,
                            ),
                            (
                                "from_source.signing_key_fingerprint",
                                &mine.signing_key_fingerprint,
                                &theirs.signing_key_fingerprint,
                            ),
                        ] {
                            if a.trim() != b.trim() {
                                problems.push(LockViolation::IdFieldMismatch {
                                    row,
                                    id: id.clone(),
                                    field,
                                });
                            }
                        }
                    }
                }
            }

            match (entry.acquisition, &entry.from_source) {
                (Acquisition::FromSource, Some(anchor)) => {
                    if !sha256_is_well_formed(&anchor.tarball_sha256) {
                        problems.push(LockViolation::MalformedSha256 {
                            row,
                            id: id.clone(),
                            field: "from_source.tarball_sha256",
                            sha256: anchor.tarball_sha256.clone(),
                        });
                    }
                    for (field, value) in [
                        (
                            "from_source.signing_key_fingerprint",
                            &anchor.signing_key_fingerprint,
                        ),
                        ("from_source.toolchain_digest", &anchor.toolchain_digest),
                    ] {
                        if value.trim().is_empty() {
                            problems.push(LockViolation::EmptyField {
                                row,
                                id: id.clone(),
                                field,
                            });
                        }
                    }
                }
                (Acquisition::Prebuilt, None) => {}
                (acquisition, _) => problems.push(LockViolation::AnchorModeMismatch {
                    row,
                    id: id.clone(),
                    acquisition,
                }),
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture lives under `tests/` per the P4.56.1 box, and is NOT the committed manifest
    /// (`src-tauri/engines.lock` is L(-1)-caged). Malformed cases are derived from it by
    /// unique-anchor substitution, so each one names the single shape it breaks.
    const VALID: &str = include_str!("../../../tests/engines-lock-valid.toml");

    /// The §3.7.2 item-4 SPLIT shape: one sub-component, two triples, one shared source anchor. Its own
    /// file so the main fixture's substitution anchors stay unique.
    const SUBCOMPONENT_SPLIT: &str =
        include_str!("../../../tests/engines-lock-subcomponent-split.toml");

    /// The §3.8 mixed-mode shape: one component prebuilt on one triple and compiled on two others. It
    /// takes THREE rows of one id to expose a baseline keyed on row 0 — see the fixture's header.
    const ANCHOR_BASELINE: &str = include_str!("../../../tests/engines-lock-anchor-baseline.toml");

    /// Substitute a UNIQUE anchor. `str::replace` is global, so an anchor occurring twice would mutate
    /// two rows and the test would assert something other than it claims — assert uniqueness first.
    fn mutate(anchor: &str, replacement: &str) -> String {
        assert_eq!(
            VALID.matches(anchor).count(),
            1,
            "the fixture anchor {anchor:?} must be unique, or the mutation hits more than one row"
        );
        VALID.replace(anchor, replacement)
    }

    /// Apply several unique-anchor substitutions in sequence. Each anchor is checked against the
    /// PRISTINE fixture, so a chain cannot quietly opt out of the uniqueness guard.
    fn mutate_all(edits: &[(&str, &str)]) -> String {
        let mut out = VALID.to_owned();
        for (anchor, replacement) in edits {
            assert_eq!(
                VALID.matches(anchor).count(),
                1,
                "the fixture anchor {anchor:?} must be unique"
            );
            out = out.replace(anchor, replacement);
        }
        out
    }

    fn parse(src: &str) -> EnginesLock {
        toml::from_str(src).expect("the fixture is well-formed TOML matching the schema")
    }

    fn violations(src: &str) -> Vec<LockViolation> {
        parse(src)
            .validate()
            .expect_err("the mutated fixture is deliberately invalid")
    }

    /// The whole §3.7.2 field list round-trips, across all four row shapes the row law admits.
    #[test]
    fn the_fixture_exercises_every_row_shape_and_validates() {
        let lock = parse(VALID);
        assert_eq!(lock.engine.len(), 4);
        assert_eq!(lock.validate(), Ok(()));

        // (1)+(2) the SAME artifact on two triples — two rows, two hashes, two availability values.
        let (linux, windows) = (&lock.engine[0], &lock.engine[1]);
        assert_eq!(linux.id, windows.id, "the same artifact");
        assert_ne!(linux.triples, windows.triples, "keyed by different triples");
        assert_ne!(linux.sha256, windows.sha256, "one row = one sha256");
        assert_eq!(linux.available, Some(true));
        assert_eq!(
            windows.available,
            Some(false),
            "§3.4.4a: the flag is a SCALAR per row, so a per-target flip is plain data"
        );

        // (3) a byte-identical platform-invariant artifact: ONE row over several triples.
        let font = &lock.engine[2];
        assert_eq!(
            font.triples.len(),
            4,
            "one row, not four copies that could drift"
        );
        assert_eq!(font.kind, RowKind::StagedArtifact);
        assert!(
            font.available.is_none(),
            "only a codec row carries the flag"
        );
        assert!(
            font.from_source.is_none(),
            "a prebuilt row carries no anchor set"
        );

        // (4) a §3.7.2 item-4 sub-component: the hash anchors the pinned SOURCE, no staged file.
        let vendored = &lock.engine[3];
        assert_eq!(vendored.kind, RowKind::SubComponent);
        assert_eq!(vendored.linkage, Linkage::Linked);
        assert_eq!(
            vendored.source_ref.len(),
            40,
            "P4.54 reads this as the pinned fork COMMIT"
        );
        let anchor = vendored
            .from_source
            .as_ref()
            .expect("a from-source row carries the §3.8/G37 anchor set");
        assert_eq!(anchor.verified_with, VerificationTool::Sq);
    }

    /// The row law FORCES a sub-component whose per-target build differs to split across triples, and
    /// those rows share one triple-invariant source anchor. The duplicate-hash rule must therefore not
    /// fire on them — otherwise the mandated shape is unwritable, which is the asymmetry the P4.56.1
    /// dual review found: `from_source.tarball_sha256` was already exempt while `sha256` was not.
    /// Both directions are pinned: the split is VALID, and flipping the pair to staged artifacts (the
    /// only thing that changes) makes it a violation.
    #[test]
    fn a_sub_component_split_across_triples_may_share_its_source_anchor() {
        let split: EnginesLock =
            toml::from_str(SUBCOMPONENT_SPLIT).expect("the split fixture matches the schema");
        assert_eq!(split.engine.len(), 2);
        assert_eq!(
            split.engine[0].sha256, split.engine[1].sha256,
            "one source anchor"
        );
        assert_ne!(
            split.engine[0]
                .from_source
                .as_ref()
                .map(|a| &a.toolchain_digest),
            split.engine[1]
                .from_source
                .as_ref()
                .map(|a| &a.toolchain_digest),
            "the per-target build container is what forks, forcing the split"
        );
        assert_eq!(
            split.validate(),
            Ok(()),
            "§3.7.2 item 4: sub-component rows share a source anchor by construction"
        );

        // The SAME two rows as staged artifacts: now the shared hash IS an unmerged pair.
        let staged: EnginesLock = toml::from_str(
            &SUBCOMPONENT_SPLIT.replace("kind = \"sub-component\"", "kind = \"staged-artifact\""),
        )
        .expect("the mutated fixture still parses");
        assert!(
            staged
                .validate()
                .expect_err("two staged artifacts sharing a hash is an unmerged pair")
                .iter()
                .any(|v| matches!(v, LockViolation::DuplicateSha256 { row: 1, .. })),
            "the rule still bites where it is meant to"
        );
    }

    /// Sibling rows of one id must agree on what identifies the COMPONENT, and must be free to differ
    /// on what identifies the per-target ARTIFACT. Both halves are asserted, because a check that only
    /// caught drift would also be satisfied by forbidding the legitimate per-triple variation the row
    /// law exists to express. [Raised by the P4.56.1 dual review]
    #[test]
    fn sibling_rows_of_one_id_must_agree_on_the_component_but_may_differ_per_target() {
        // Drift: the same component pinned at two versions across triples.
        let drifted = mutate(
            "version = \"3.6\"
source_ref = \"3.6\"
triples = [\"x86_64-pc-windows-msvc\"]",
            "version = \"3.5\"
source_ref = \"3.6\"
triples = [\"x86_64-pc-windows-msvc\"]",
        );
        let problems = parse(&drifted)
            .validate()
            .expect_err("two rows of one id at different versions is drift");
        assert!(
            problems.contains(&LockViolation::IdFieldMismatch {
                row: 1,
                id: "libheif-x265-plugin".to_owned(),
                field: "version"
            }),
            "got: {problems:?}"
        );

        // Legitimate per-target variation on the SAME id — the pristine fixture already carries it.
        let lock = parse(VALID);
        let (a, b) = (&lock.engine[0], &lock.engine[1]);
        assert_eq!(a.id, b.id);
        assert_ne!(a.sha256, b.sha256, "per-target bytes differ");
        assert_ne!(a.available, b.available, "§3.4.4a flips per target");
        assert!(
            a.cpe.is_some() && b.cpe.is_none(),
            "cpe is optional per row"
        );
        assert_eq!(lock.validate(), Ok(()), "none of that is drift");
    }

    /// The sibling lookup must key on the TRIMMED id, exactly as `DuplicateKey` does. With the RAW id
    /// pushed, a stray space on the FIRST row of an artifact mints an identity its siblings never
    /// match, so the whole component-consistency check goes silently dead — while `DuplicateKey` still
    /// binds the pair as one artifact, leaving the two checks disagreeing about what "the same row"
    /// means. The space must sit on the FIRST row: on any subsequent one the trimmed lookup still
    /// matches, and the leg would pass against the bug (verified by patching the revert).
    /// [Raised by the P4.56.1 dual review]
    #[test]
    fn a_stray_space_on_the_first_row_of_an_id_cannot_disable_the_sibling_check() {
        let drifted = mutate_all(&[
            (
                "id = \"libheif-x265-plugin\"
version = \"3.6\"
source_ref = \"3.6\"
triples = [\"x86_64-unknown-linux-gnu\"]",
                "id = \"libheif-x265-plugin \"
version = \"3.6\"
source_ref = \"3.6\"
triples = [\"x86_64-unknown-linux-gnu\"]",
            ),
            (
                "id = \"libheif-x265-plugin\"
version = \"3.6\"
source_ref = \"3.6\"
triples = [\"x86_64-pc-windows-msvc\"]",
                "id = \"libheif-x265-plugin\"
version = \"3.5\"
source_ref = \"3.6\"
triples = [\"x86_64-pc-windows-msvc\"]",
            ),
        ]);
        let problems = parse(&drifted)
            .validate()
            .expect_err("a whitespace-only id difference is the SAME artifact, and it drifted");
        assert!(
            problems.contains(&LockViolation::IdFieldMismatch {
                row: 1,
                id: "libheif-x265-plugin".to_owned(),
                field: "version"
            }),
            "got: {problems:?}"
        );
    }

    /// §3.6.1's copyleft-isolation class is component identity, not a per-target fact: a sibling pair
    /// claiming `invoked` on one triple and `linked` on another would be a silent change to the
    /// aggregation argument CLAUDE §3's MIT-core-clean guardrail rests on. Both reviewers read §3.6.1
    /// the same way — the "PER engine PER platform" qualifier §3.8 gives `acquisition` appears nowhere
    /// near linkage. [Raised by the P4.56.1 dual review]
    #[test]
    fn a_linkage_that_forks_between_sibling_rows_is_a_violation() {
        let drifted = mutate(
            "linkage = \"plugin-loaded\"
acquisition = \"from-source\"
purl = \"pkg:generic/x265@3.6\"
sha256 = \"3333",
            "linkage = \"invoked\"
acquisition = \"from-source\"
purl = \"pkg:generic/x265@3.6\"
sha256 = \"3333",
        );
        assert!(parse(&drifted)
            .validate()
            .expect_err("a forked copyleft class is drift")
            .contains(&LockViolation::IdFieldMismatch {
                row: 1,
                id: "libheif-x265-plugin".to_owned(),
                field: "linkage"
            }));
    }

    /// The same `source_ref` entails the same signed tarball, so the same hash and key — the xz-class
    /// provenance anchor. Compared only when BOTH siblings are from-source, since `acquisition`
    /// legitimately forks per triple. [Raised by the P4.56.1 dual review]
    #[test]
    fn a_from_source_anchor_that_forks_between_sibling_rows_is_a_violation() {
        let drifted = mutate(
            "tarball_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"
signing_key_fingerprint = \"AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555\"
verified_with = \"gpg\"
toolchain_digest = \"sha256:4444",
            "tarball_sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"
signing_key_fingerprint = \"AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555\"
verified_with = \"gpg\"
toolchain_digest = \"sha256:4444",
        );
        assert!(parse(&drifted)
            .validate()
            .expect_err("one source ref cannot have two signed tarballs")
            .contains(&LockViolation::IdFieldMismatch {
                row: 1,
                id: "libheif-x265-plugin".to_owned(),
                field: "from_source.tarball_sha256"
            }));
    }

    /// The anchor comparison must not depend on which acquisition mode sorts FIRST. With a prebuilt row
    /// ahead of two from-source siblings, a baseline keyed on row 0 of the id finds no anchor and the
    /// comparison silently no-ops — so two rows naming one `source_ref` could carry two different signed
    /// tarballs, the xz-class drift the field was added to catch. §3.8 makes mixed modes legitimate, so
    /// this ordering is a normal manifest, not a contrived one. [Raised by the P4.56.1 dual review]
    #[test]
    fn the_anchor_comparison_is_baselined_on_the_first_row_that_has_an_anchor() {
        let lock: EnginesLock =
            toml::from_str(ANCHOR_BASELINE).expect("the mixed-mode fixture matches the schema");
        assert_eq!(lock.engine.len(), 3);
        assert!(
            lock.engine[0].from_source.is_none(),
            "the prebuilt row sorts FIRST and carries no anchor — that is the whole point"
        );
        let problems = lock
            .validate()
            .expect_err("one source_ref cannot have two signed tarballs");
        assert!(
            problems.contains(&LockViolation::IdFieldMismatch {
                row: 2,
                id: "libmp3lame".to_owned(),
                field: "from_source.tarball_sha256"
            }),
            "the divergence must be caught behind the prebuilt row; got: {problems:?}"
        );
        // Nothing ELSE is reported: the mixed `acquisition` and the per-target hashes are legitimate.
        assert_eq!(
            problems.len(),
            1,
            "no false positives from the mixed mode: {problems:?}"
        );
    }

    /// An EMPTY manifest is valid — the container exists from P4.56.1 and fills at P5–P7.
    #[test]
    fn an_empty_manifest_is_a_valid_container() {
        let lock: EnginesLock = toml::from_str("").expect("an empty manifest parses");
        assert!(lock.engine.is_empty());
        assert_eq!(lock.validate(), Ok(()));
    }

    /// A MISSING required field is a parse error, not a silently-defaulted row. `source_ref` is the
    /// one §3.8 requires ("an exact version + source ref") and G37 restates in bold.
    #[test]
    fn a_missing_required_field_fails_to_parse() {
        for anchor in [
            "source_ref = \"f8b6bcf1e0ff0e6a5f2f6cd8bbd0da4c92a5e4bd\"\n",
            "kind = \"sub-component\"\n",
            "triples = [\"x86_64-pc-windows-msvc\"]\n",
        ] {
            let without = mutate(anchor, "");
            assert!(
                toml::from_str::<EnginesLock>(&without).is_err(),
                "a row without {anchor:?} must not parse"
            );
        }
    }

    /// A TYPO'd key must fail rather than being dropped — the reason `deny_unknown_fields` is on.
    #[test]
    fn a_typoed_key_fails_rather_than_being_silently_dropped() {
        let typo = mutate(
            "signing_key_fingerprint = \"FFFF",
            "signing_key_fingerprnt = \"FFFF",
        );
        let err = toml::from_str::<EnginesLock>(&typo)
            .expect_err("an unknown key must be rejected, not ignored");
        let text = err.to_string();
        assert!(
            text.contains("signing_key_fingerprnt") || text.contains("unknown field"),
            "the error should name the offending key, got: {text}"
        );
    }

    /// The closed vocabularies reject an unknown value — §3.6.1's linkage, the row kind, and G37's
    /// named verification tool.
    #[test]
    fn the_closed_enums_reject_an_unknown_value() {
        for (from, to) in [
            ("linkage = \"linked\"", "linkage = \"dlopened\""),
            ("kind = \"sub-component\"", "kind = \"vendored\""),
            ("verified_with = \"sq\"", "verified_with = \"eyeball\""),
        ] {
            assert!(
                toml::from_str::<EnginesLock>(&mutate(from, to)).is_err(),
                "{to} is not a member of its closed set"
            );
        }
    }

    /// §3.4.4a's flag is now a SCALAR: the retired `{win, macos, linux}` table shape must NOT parse,
    /// which is what makes the literal→normative reconciliation real rather than merely documented.
    #[test]
    fn the_retired_per_platform_availability_table_no_longer_parses() {
        let table = mutate(
            "available = true",
            "available = { win = true, macos = true, linux = false }",
        );
        assert!(
            toml::from_str::<EnginesLock>(&table).is_err(),
            "§3.4.4a row-law: availability is a scalar on a triple-keyed row"
        );
    }

    #[test]
    fn a_blank_required_value_is_a_violation() {
        assert!(
            violations(&mutate("licence = \"OFL-1.1\"", "licence = \"  \"")).contains(
                &LockViolation::EmptyField {
                    row: 2,
                    id: "LiberationSans-Regular.ttf".to_owned(),
                    field: "licence"
                }
            )
        );
    }

    /// The row key: the SAME artifact on the SAME triple twice is what "one row = one sha256" forbids.
    #[test]
    fn a_repeated_artifact_triple_key_is_a_violation() {
        let dup = mutate(
            "triples = [\"x86_64-pc-windows-msvc\"]",
            "triples = [\"x86_64-unknown-linux-gnu\"]",
        );
        assert!(
            violations(&dup).contains(&LockViolation::DuplicateKey {
                row: 1,
                id: "libheif-x265-plugin".to_owned(),
                triple: "x86_64-unknown-linux-gnu".to_owned()
            }),
            "(artifact, triple) must map to exactly one row"
        );
    }

    /// Two rows with the same hash mean a byte-identical artifact was NOT merged into one multi-triple
    /// row — two rows that can drift apart under the next hand edit.
    #[test]
    fn two_rows_sharing_a_hash_is_a_violation() {
        let same = mutate(
            "sha256 = \"3333333333333333333333333333333333333333333333333333333333333333\"",
            "sha256 = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"",
        );
        assert!(violations(&same)
            .iter()
            .any(|v| matches!(v, LockViolation::DuplicateSha256 { row: 1, .. })));
    }

    /// The same PURL on several rows is legitimate — one component staged per triple — so the purl
    /// carries no uniqueness rule. Pinned so the duplicate-hash rule above is not over-generalised.
    #[test]
    fn the_same_purl_on_several_rows_is_legitimate() {
        let lock = parse(VALID);
        assert_eq!(
            lock.engine[0].purl, lock.engine[1].purl,
            "one component, two triples, two rows — same purl by construction"
        );
        assert_eq!(lock.validate(), Ok(()));
    }

    /// A triple outside the §3.4.5 v1 set — including `universal-apple-darwin`, which is a `lipo`
    /// BUILD OUTPUT and never a row key.
    #[test]
    fn an_unknown_or_universal_triple_is_a_violation() {
        for bad in ["universal-apple-darwin", "x86_64-unknown-linux-musl"] {
            let mutated = mutate(
                "triples = [\"x86_64-pc-windows-msvc\"]",
                &format!("triples = [\"{bad}\"]"),
            );
            assert!(
                violations(&mutated).iter().any(
                    |v| matches!(v, LockViolation::UnknownTriple { triple, .. } if triple == bad)
                ),
                "{bad} is not a §3.4.5 row key"
            );
        }
    }

    #[test]
    fn a_row_with_no_triples_is_a_violation() {
        let none = mutate("triples = [\"x86_64-pc-windows-msvc\"]", "triples = []");
        assert!(violations(&none)
            .iter()
            .any(|v| matches!(v, LockViolation::NoTriples { row: 1, .. })));
    }

    /// The §3.7.2 purl minimum, proved by the shapes that must FAIL as well as the one that passes.
    #[test]
    fn the_purl_minimum_is_enforced() {
        assert_eq!(purl_version("pkg:generic/ffmpeg@7.1"), Some("7.1"));
        for bad in [
            "ffmpeg@7.1",           // no purl scheme/type
            "pkg:cargo/ffmpeg@7.1", // §3.7.2 fixes the `generic` type for staged artifacts
            "pkg:generic/ffmpeg",   // no version
            "pkg:generic/@7.1",     // no name
            "pkg:generic/ffmpeg@",  // empty version
        ] {
            assert!(
                purl_version(bad).is_none(),
                "{bad} must not satisfy the §3.7.2 minimum"
            );
        }
    }

    /// A purl whose version disagrees with the row's own would match NOTHING in G17b's CVE lookup — a
    /// green-but-empty report reading as "no known CVEs".
    #[test]
    fn a_purl_version_that_disagrees_with_the_row_is_a_violation() {
        let drift = mutate(
            "purl = \"pkg:generic/liberation-fonts@2.1.5\"",
            "purl = \"pkg:generic/liberation-fonts@2.1.4\"",
        );
        assert!(
            violations(&drift).contains(&LockViolation::PurlVersionMismatch {
                row: 2,
                id: "LiberationSans-Regular.ttf".to_owned(),
                purl_version: "2.1.4".to_owned()
            })
        );
    }

    /// The hash format, including the case rule: an UPPERCASE hash would compare unequal to a tool's
    /// lowercase output and read as tampering rather than as a formatting slip.
    #[test]
    fn the_sha256_format_is_64_lowercase_hex() {
        let ok = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(sha256_is_well_formed(ok));
        assert!(!sha256_is_well_formed(&ok.to_uppercase()), "case is fixed");
        assert!(!sha256_is_well_formed(&ok[..63]), "too short");
        assert!(!sha256_is_well_formed(&format!("{ok}0")), "too long");
        assert!(!sha256_is_well_formed(&ok.replacen('0', "g", 1)), "non-hex");
    }

    /// The anchor's tarball hash is held to the same format, under its own field name so a failure
    /// says WHICH hash is malformed.
    #[test]
    fn the_from_source_tarball_hash_is_format_checked_under_its_own_field_name() {
        let bad = mutate(
            "tarball_sha256 = \"7777777777777777777777777777777777777777777777777777777777777777\"",
            "tarball_sha256 = \"deadbeef\"",
        );
        assert!(violations(&bad).contains(&LockViolation::MalformedSha256 {
            row: 3,
            id: "libimagequant".to_owned(),
            field: "from_source.tarball_sha256",
            sha256: "deadbeef".to_owned()
        }));
    }

    /// A prefix-only CPE check would accept `cpe:2.3:junk`, which matches nothing in a CVE lookup while
    /// LOOKING like coverage — the same failure mode a wrong purl has.
    #[test]
    fn the_cpe_must_be_a_full_cpe_2_3_string_not_just_the_prefix() {
        assert!(cpe_is_well_formed(
            "cpe:2.3:a:multicorewareinc:x265:3.6:*:*:*:*:*:*:*"
        ));
        for bad in [
            "cpe:2.3:junk",                               // prefix only
            "cpe:2.3:a:vendor:product:1.0:*:*:*:*:*:*",   // 12 components
            "cpe:2.3:z:vendor:product:1.0:*:*:*:*:*:*:*", // part not a/o/h
            "cpe:2.2:a:vendor:product:1.0:*:*:*:*:*:*:*", // wrong version
        ] {
            assert!(!cpe_is_well_formed(bad), "{bad} is not a CPE 2.3 string");
        }
        let bad = mutate(
            "cpe = \"cpe:2.3:a:multicorewareinc:x265:3.6:*:*:*:*:*:*:*\"",
            "cpe = \"cpe:2.3:junk\"",
        );
        assert!(violations(&bad)
            .iter()
            .any(|v| matches!(v, LockViolation::MalformedCpe { row: 0, .. })));
        // A row with NO cpe stays clean — "where one exists" is not "always".
        assert!(parse(VALID).engine[2].cpe.is_none());
    }

    /// P4.56.3 compares ORIGINS, so a scheme-less URL has nothing to compare. Both URL fields are
    /// driven, since each is read by a different half of that gate.
    #[test]
    fn a_schemeless_url_is_a_violation_on_both_url_fields() {
        assert!(url_has_scheme("https://ffmpeg.org/x"));
        assert!(!url_has_scheme("ffmpeg.org/x"));
        assert!(!url_has_scheme("://ffmpeg.org"));
        for (field, anchor, replacement) in [
            (
                "corroboration_url",
                "corroboration_url = \"https://github.com/lovell/libimagequant/releases/tag/v2.4.1\"",
                "corroboration_url = \"github.com/lovell/libimagequant/releases/tag/v2.4.1\"",
            ),
            (
                "upstream_url",
                "upstream_url = \"https://github.com/lovell/libimagequant\"",
                "upstream_url = \"github.com/lovell/libimagequant\"",
            ),
        ] {
            assert!(
                violations(&mutate(anchor, replacement))
                    .iter()
                    .any(|v| matches!(v, LockViolation::SchemelessUrl { field: f, .. } if *f == field)),
                "{field} must be origin-derivable"
            );
        }
    }

    /// §3.8's two modes have different ground truths, so the anchor set is required by exactly one of
    /// them — both directions asserted.
    #[test]
    fn the_from_source_anchor_set_is_required_by_mode_in_both_directions() {
        let missing = mutate(
            "acquisition = \"prebuilt\"",
            "acquisition = \"from-source\"",
        );
        assert!(
            violations(&missing).contains(&LockViolation::AnchorModeMismatch {
                row: 2,
                id: "LiberationSans-Regular.ttf".to_owned(),
                acquisition: Acquisition::FromSource
            })
        );

        let spurious = mutate(
            "acquisition = \"from-source\"\npurl = \"pkg:generic/libimagequant@2.4.1\"",
            "acquisition = \"prebuilt\"\npurl = \"pkg:generic/libimagequant@2.4.1\"",
        );
        assert!(
            violations(&spurious).contains(&LockViolation::AnchorModeMismatch {
                row: 3,
                id: "libimagequant".to_owned(),
                acquisition: Acquisition::Prebuilt
            })
        );
    }

    /// The validator reports EVERY problem in one pass — a hand-edited manifest under owner-ack should
    /// need one round-trip, not one per defect.
    #[test]
    fn validation_collects_every_violation_rather_than_failing_fast() {
        let broken = mutate_all(&[
            ("licence = \"OFL-1.1\"", "licence = \"\""),
            (
                "purl = \"pkg:generic/liberation-fonts@2.1.5\"",
                "purl = \"liberation-fonts\"",
            ),
            (
                "sha256 = \"5555555555555555555555555555555555555555555555555555555555555555\"",
                "sha256 = \"deadbeef\"",
            ),
        ]);
        let problems = parse(&broken)
            .validate()
            .expect_err("three independent defects in one row");
        assert_eq!(
            problems.len(),
            3,
            "one pass must surface all three, got: {problems:?}"
        );
    }
}
