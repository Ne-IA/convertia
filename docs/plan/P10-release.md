# P10 — Release Mechanics (technical — downloads only)

> **Goal:** a user can **download a verified build** — the technical machinery only.
> P10 **builds the release plane** whose **policy P0.7 already authored**: the per-OS
> bundles, the SBOM finalize + attribution-completeness gate, the checksums + the
> minisign signature over `SHA256SUMS`, build-provenance attestation, reproducible-ish
> evidence, the tag-triggered **Lane-B GitHub Releases pipeline**, the
> download/trust-page content, the artifact-size gate, the no-system-pollution gate,
> and the governance/clearance/release-completeness release-blocking gates. **There is
> NO auto-update / phone-home:** `tauri-plugin-updater` is **absent** and P10 wires the
> structural assertion of that absence (§7.6.1 — "its absence is the implementation").
>
> **Spec home:** [`06-build-test-release.md`](../spec/06-build-test-release.md) (§6.2
> integrity/checksums/minisign + §6.2.4 download-trust page + §6.2.5 reproducible-build
> intent, §6.3 SBOM + completeness gate, §6.7.2 Lane-B release pipeline + §6.7.2
> size gate, §6.8 governance gate, §6.9 name/clearance gate + rename, §6.10 row 21
> no-system-pollution gate / row 22 size gate / rows 9/10/12/16/17 release artifacts),
> [`07-app-shell.md`](../spec/07-app-shell.md) (§7.6 no-update posture).
> Box format: [`_format.md`](_format.md). Index: [README.md](README.md).
>
> **This is the v0 base** — the smallest atomic `[ ]` boxes below, grouped under
> `### ` sub-headings; a later adversarial-review pass deepens, splits and reconciles
> them (incl. P0.7's `→ executed in P10` cross-refs against these real box-ids). When
> in doubt the boxes are made **smaller and more numerous**, never coarser.

## Boundaries (read against P0.7, P4–P9, P11)

- **P0.7 ↔ P10 (the controlling boundary):** **P0.7 authored the policy + acceptance
  criteria + the config/schema/docs** for every release-plane gate (G35/G35a/G35b/G36/
  G36b SBOM, G37/G37b/G37c/G38/G38b/G41b engine staging, G39/G44 minisign+verify-recipe,
  G55 auditable binary, G58 completeness meta-gate, G59 provenance, G60 reproducibility,
  G41 size, G43 no-pollution, G44 governance, G45 clearance, G33b contrast, G64 ratchet,
  G17b CVE-awareness, G46 startup-acceptance). **P10 WIRES + EXECUTES them in the Lane-B
  pipeline** — no policy is re-decided here, no double-build. Each P10 box that activates
  a P0.7-authored gate `needs:` the P0.7 box that authored it.
- **P4–P7 ↔ P10:** the **per-engine** G37/G37b/G37c/G38/G38b assertions and the
  **per-engine SBOM/NOTICE rows** are **executed per-engine as each is staged in P4–P7**
  (the `→ executed in P4–P7` annotation in P0.7.3/P0.7.4/P0.7.1). P10 does **NOT** re-run
  a per-engine `pandoc --version` assertion; P10 **assembles** the finalized SBOM across
  all populated rows, runs the **merge + completeness gate** over the whole bundle, and
  runs the **whole-bundle** Syft cross-check / static-link-closure derivation.
- **P9 ↔ P10:** the **offline-egress / read-half observability gate** (G42/G42b) is
  **built in P9** (§6.7.3). P10 only **enumerates** its evidence in the release-artifact
  completeness meta-gate; the **no-system-pollution gate (G43)** is the one observability
  gate **built in P10** (§6.10 row 21, the Lane-B post-launch state-snapshot-diff).
- **P10 ↔ P11:** P10 **builds** the Lane-B pipeline + every release-blocking gate and
  **wires the usability-floor *evidence* gate** (Lane-B stage 5 reads
  `docs/usability-floor.md`); **P11 performs** the §6.6 human walkthroughs that *produce*
  that evidence and proves the assembled gates green for the RC. P10 builds machinery;
  P11 proves it. No double-build.

---

### No-update posture (assert the updater is absent)

- [ ] **P10.1** [GATE] Wire the `tauri-plugin-updater`-absent structural assertion into the release lint · §7.6.1 §0.10 · G47
  needs: P0.3.2, P9.48
  > extend the G47 CSP/capability structural lint (P0.3.2) so the **release plane** re-asserts §7.6.1's "absence is the implementation": **no** `tauri-plugin-updater` in `Cargo.toml`/the Builder, **no** `updater` bundle block / `bundle.createUpdaterArtifacts` / updater pubkey in `tauri.conf.json`, **no** update endpoint/manifest, **no** remote origin in any CSP directive or capability — fail-closed at release. The §6.7.1 "verified by the type/config checks" claim, asserted at the release tier (not only per-push).
- [ ] **P10.2** [RELEASE] Assert no background/startup version-check + no "you're up to date" surface ships · §7.6.1 §7.6.2 · G44
  needs: P10.1
  > a release-tier assertion that the shell makes **zero** network calls for version-checking (§7.6.1: no startup/background fetch, no banner), and that the **only** release-discovery surface is the §7.6.2 **user-initiated** About→Releases `tauri-plugin-opener` shell-out (built in P8) — not an in-app fetch/parse of the releases page. Folds into the §6.8 governance/download-page assertion (G44).
- [ ] **P10.3** [GATE] Re-assert the `tauri-plugin-updater` `cargo-deny [bans]` deny-leg at the release tier · §7.6.1 §3.8 · G18
  needs: P0.3.6
  > the P0.3.6 `deny.toml` `[bans]` already denies `tauri-plugin-updater` (+ the HTTP-client crates) per-push; this box wires the **release-pipeline** re-run of `cargo deny check bans --locked` so a last-minute `cargo add tauri-plugin-updater` on the release commit is caught on the tag build, not only on a normal push (defence in depth behind G47/P10.1).

---

### Lane-B release pipeline skeleton & ordering

- [ ] **P10.4** [CI] Build the tag-triggered Lane-B release workflow from the P0.2.5 skeleton — fill the empty stage slots · §6.7.2 · G58 G56
  needs: P0.2.5, P0.2.9, P0.7.17
  > take the `v*`-tag-triggered Lane-B skeleton (P0.2.5) + the P0.2.9 first-step trust assertion (tagged commit is an `origin/main` ancestor + main's required checks were green for that SHA + `git verify-tag`, abort **before** any secret is read) and fill the **seven ordered stages** §6.7.2 enumerates (each blocking the next): matrix build → reliability gate → SBOM+completeness → clearance → usability-floor evidence → integrity-hash+sign → publish. The stage *contents* are the boxes below; this box stands up the workflow shape + the strict stage ordering + the per-job `timeout-minutes`.
  > **`needs: P0.7.17`** because the P0.2.9 `git verify-tag` first-step is only **fail-closed** (its G56b leg-3 target posture) once the committed **`.github/allowed_signers`** exists — so the loop must not stand up the release workflow until the owner has provisioned the SSH signing key (P0.7.17). This is the **tag-ref STOP**, symmetric with **P10.6's `needs: P0.7.18`** Environment STOP — together they guarantee no release is minted until both the signing key (tag trust) and the approval-gated secret (key custody) are provisioned (release-pipeline-trust.md §5).
- [ ] **P10.5** [CI] Wire the Lane-B stage-ordering gate — each stage hard-blocks the next, no stage skips on a soft failure · §6.7.2 · G58
  needs: P10.4
  > assert the §6.7.2 "**in order, each blocking the next**" contract structurally: a failing earlier stage **aborts** the release (no `continue-on-error` on a release-blocking stage), and a stage cannot run before its predecessor's success. The release is all-or-nothing (one coordinated release, SSOT) — a partial publish is forbidden.
- [ ] **P10.6** [CI] Provision the `release` GitHub Environment binding + the human-approval gate on the signing stage · §6.7.2 · G56
  needs: P0.7.18, P10.4
  > execute the P0.7.18 policy: bind the secret-bearing signing job with `environment: release` (required-reviewers + the `v*` deployment-branch/tag policy) so `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD` are **never injected until a human approves** the one irreversible action; the G56 `gh api …/environments/release` assertion (required_reviewers + `v*` deployment_branch_policy + the binding) flips from the P0 fail-soft bootstrap to **hard**. Release-job token scope `contents: write` ONLY (+ `id-token: write` only on the attestation step), never on a fork PR.
- [ ] **P10.7** [CI] Bind the secret-bearing signing job to an ephemeral GitHub-hosted runner host-disjoint from the Lane-B corpus/fuzz legs · §6.7.2 §6.1.4 · G56
  needs: P0.2.7, P10.4
  > execute P0.2.7/P0.7.5's runner-host-integrity policy at the pipeline: the integrity-hash+sign stage runs on an **ephemeral GitHub-hosted runner** under `step-security/harden-runner` (BLOCK), **never** the shared self-hosted VPS that ran the stage-2 untrusted-corpus/fuzz Linux leg; assert the signing job and the corpus/fuzz job declare **disjoint hosts / no shared workspace** (G56 fails a secret-using job on a self-hosted label). Implements security-concept principle 11. (Precondition: the §6.1.4 self-hosted VPS that runs the disjoint corpus/fuzz Linux leg is the owner-provisioned runner recorded by **P9.47** — this signing-host disjointness assertion presumes that runner exists for the stage-2 leg; it is not a hard `needs: P9.47` since the disjointness check is structural over the workflow YAML.)
  - [ ] **P10.7.1** [CI] Assert the signing job's `CARGO_NET_OFFLINE=true`-after-`cargo fetch --locked` posture (the P0.4.4 build.rs/proc-macro execution-isolation contract) · §3.8 §6.7.2 · G56
    needs: P0.4.4
    > execute the P0.4.4 contract at the pipeline: a `jq`/`yq`-over-parsed-YAML sub-assertion that the secret-bearing signing job (a) runs an explicit `cargo fetch --locked` **before** any `cargo build`/`test`, (b) sets `CARGO_NET_OFFLINE=true` in its `env:` so a build.rs/proc-macro cannot phone home to exfiltrate `MINISIGN_SECRET_KEY` after the fetch, and (c) has **no** `cargo build`/`test` step without `CARGO_NET_OFFLINE` in scope — the G56 no-network-after-fetch sub-rule. Reinforced by the harden-runner BLOCK (P10.7); P0.4.4 is the policy source.

---

### Per-OS bundle build & packaging (Lane-B stage 1)

- [ ] **P10.8** [BUILD] Execute the cross-platform native build matrix — three native legs, no cross-compile · §6.1.1 §6.1.4 · G30
  needs: P0.4.10, P10.4
  > the Lane-B stage-1 matrix build on the three native legs (`windows-latest`, `macos-latest` arm64 building `universal-apple-darwin`, pinned `ubuntu-22.04` / the self-hosted VPS path) per §6.1.4; activates the P0.4.10 build-matrix contract. No cross-compile (§6.1.1). Per-leg toolchain (Rust host triple, both macOS targets via `rustup target add`, Node+pnpm) installed through the P0.2.1 pinned fetch-and-verify mechanism.
- [ ] **P10.9** [BUILD] Run `scripts/stage-engines` per leg from the verified engine-asset cache, then `tauri build` · §6.1.3 · G37
  needs: P10.8
  > stage each platform's engines from the `actions/cache` `<engine>-<version>-<triple>` cache (checksum-verified pinned-URL fetch on a miss, re-verified vs `engines.lock` on cache-restore, §6.1.3/§6.3.4) into `src-tauri/binaries/` + `src-tauri/resources/`, then run `tauri build`. Activates the per-engine G37 SHA-verify-before-stage policy (authored P0.7.4, executed per-engine P4–P7) at the whole-bundle release build. A platform ships only the engines §3.4 makes available there — never a silent omission.
- [ ] **P10.10** [BUILD] Execute the macOS per-sidecar `lipo -create` universal-fat-binary assembly + the per-sidecar `lipo -info` assertion · §6.1.3 §6.1.4 · G30
  needs: P10.9, P4.29, P4.29.1
  > on the macOS leg, `stage-engines` restores **both** the `aarch64-apple-darwin` AND `x86_64-apple-darwin` slices per sidecar/lib (dual-arch cache keys) and `lipo -create`s each into one `<name>-universal-apple-darwin` fat Mach-O **before** `tauri build` (Tauri does NOT lipo sidecars), executing the **P4.29 lipo step + the P4.29.1 missing-`x86_64`-slice from-source/cross-Rosetta fallback** when the cache lacks a slice. The G30 per-sidecar `lipo -info` fat-Mach-O assertion (P0.4.10) fails the leg if a slice is missing rather than shipping a single-arch sidecar that crashes on the other arch. (`needs: P4.29/P4.29.1` — the P4-built lipo + missing-slice fallback this release leg executes.)
- [ ] **P10.11** [BUILD] Build the Windows portable `.zip` packaging step (exe + `binaries/` + `resources/` trees) — the canonical, only v1 Windows artifact · §6.1.2 · G30
  needs: P10.9
  > the **explicit post-build packaging step** (`scripts/stage-engines` + zip) producing the portable `.zip` containing the app `.exe` beside its sidecar engine trees ("download, unzip, run") — **NOT** the bare `app`/raw-`.exe` target (which omits the sidecars) and **NOT** `nsis` (NOT shipped v1, `[DECIDED-6.1a]`). The single canonical Windows artifact.
- [ ] **P10.12** [BUILD] Produce the macOS universal `.dmg` + the Linux AppImage as the canonical per-platform artifacts · §6.1.2 · G30
  needs: P10.9
  > the `app`→`dmg` target wrapping the universal `ConvertIA.app` (macOS) and the `appimage`-only Linux target (`.deb`/`.rpm` not shipped v1, `[DECIDED-6.1b]`) — one canonical portable-first artifact per platform (DoD row 13).
- [ ] **P10.13** [BUILD] Record the `latest`-runner drift facts as release-asset lines + the deployment-target / WebView2 floor assertions · §6.1.4 · G30
  needs: P10.8
  > each macOS/Windows leg records the resolved image label + Xcode/CLT (macOS) / WebView2 (Windows) version as a release-asset line, and the build **fails** if `MACOSX_DEPLOYMENT_TARGET` drifts below the §0.3.1 floor (`= 11.0`) or WebView2 is absent on the image — so a `latest`-runner roll surfaces loudly, not silently (the drift-guard price of not hard-pinning macOS/Windows).
  > **Forward-note (2026-07-22, the P3.73 fuzz round-3 finding):** the assertion is scoped to the RELEASE/shipped-build legs — never a blanket workflow-wide `MACOSX_DEPLOYMENT_TARGET` scan: `fuzz.yml`'s G48 step legitimately pins `12.0` (Apple ld-prime rejects the ASAN'd rlib initializer format below 12 under dead-strip; a CI-only build, no shipped artifact), so a blanket `== 11.0` grep over `.github/workflows/**` would false-positive against it.

---

### SBOM finalize, NOTICE assembly & attribution-completeness gate (Lane-B stage 3)

- [ ] **P10.14** [RELEASE] Execute `cargo xtask sbom` — merge the Rust + frontend + `engines.lock` layers at `specVersion 1.5` · §6.3.1 · G35
  needs: P0.7.1, P10.9
  > finalize the SBOM by running the P0.7.1-authored `cargo xtask sbom`: it shells out to `cargo cyclonedx --spec-version 1.5` (Rust) + `@cyclonedx/cdxgen --spec-version 1.5` (frontend `pnpm-lock.yaml` — NOT the npm-only `cyclonedx-npm`) + converts the now-populated `engines.lock` rows, merges into one CycloneDX JSON, and **aborts on a schema-version mismatch**. Activates G35 over the whole release bundle (per-engine rows were populated P4–P7).
- [ ] **P10.15** [RELEASE] Run the Syft staged-bundle completeness cross-check + the deterministic stage-tree file-manifest diff · §6.3.1 §6.3.3 · G35
  needs: P10.14
  > Syft scans the staged bundle and the gate **fails** if any shipped `.so`/`.dll`/`.dylib`/resource/font has no `engines.lock`/SBOM component (the §6.3.3.1 "no shipped file without an SBOM entry" rule + the T3a side-loaded-mismatch guard), backed by a deterministic stage-tree manifest diff. The whole-bundle execution of the P0.7.1 completeness policy.
- [ ] **P10.16** [RELEASE] Derive + assert the static-link SBOM closure for the imgworker stack · §6.3.1 §3.6 · G35a
  needs: P10.14
  > the `convertia-imgworker` STATICALLY links libvips+libheif+libde265+librsvg+libimagequant+ImageMagick+x265 (none in `Cargo.lock`), so the G35 static-link-blind-spot is closed by deriving the static-link closure (G35a) and asserting every statically-linked component appears as an SBOM component — the release-tier execution of the P0.7.9-adjacent derived-closure policy.
- [ ] **P10.17** [RELEASE] Generate the CycloneDX→SPDX export as a convenience release asset · §6.3.1 · G35
  needs: P10.14
  > produce the ISO-standard SPDX-JSON via the CycloneDX CLI `convert --output-format spdxjson` (fallback `syft convert`); a convenience asset, **not** the gate input (the §6.3.3 completeness gate reads the canonical CycloneDX JSON, P10.15/P10.18).
- [ ] **P10.18** [RELEASE] Generate `NOTICE` + `THIRD-PARTY-LICENSES.txt` from `engines.lock` + the SBOM (never hand-drifted) · §6.3.2 · G35 G36
  needs: P10.14
  > assemble the repo `NOTICE` + the longer `THIRD-PARTY-LICENSES.txt` from the same `engines.lock` + dependency SBOM (so they cannot drift from what ships): per engine the name+version, full licence text, and for GPL/LGPL/AGPL the **written offer of source** (pinned upstream tag + build recipe). The in-repo file and the in-bundle copy are the **same generated artifact** (§5.9 displays it).
  > **Ordering constraint (P2.98 — compile-time embed):** the §5.9/C11 `AppInfo.third_party_notice` embeds `THIRD-PARTY-LICENSES.txt` at **compile time** via `include_str!` (P2.98), so the About/embedded copy is frozen at the Rust compile — but the compile (P10.9, Lane-B stage 1) currently runs **before** this generation (P10.18, stage 3), so a naive pipeline would ship a **STALE** embedded notice while the bundle carries the fresh file (the "same generated artifact" claim above holds only if the embed is fresh). The §6.3.3 completeness gate (P10.19) reads the FILE + SBOM, **not** the compiled binary's embedded string, so nothing currently catches this. Two fixes (decide at P10.5/P10.18), the first the more direct: **(a)** extend the §6.3.3 gate to assert the shipped binary's embedded notice **==** the generated file (catches a stale embed even if generation stays a later stage); or **(b)** re-home the compile (P10.9) to run **after** generation (P10.18) — a structural stage re-order, not merely the P10.5 next-blocks-prior assertion.
- [ ] **P10.19** [RELEASE] Run the attribution-completeness release gate — copyleft text present + SPDX resolved + no MIT-taint · §6.3.3 · G36 G36b
  needs: P10.18, P10.15
  > the §6.3.3 release-blocking check (same status as the no-harm guarantee): every copyleft (GPL/LGPL/MPL/AGPL) SBOM component has its licence text in `THIRD-PARTY-LICENSES.txt` (+ a GPL-family written-offer); **no** component has an unresolved SPDX id (`UNKNOWN`/`NOASSERTION` = hard fail, with the `LicenseRef-…`-with-text carve-out, e.g. `LicenseRef-AOMPL-1.0`); **no** engine that would taint the MIT core via linking slipped in. G36 (Rust+bundled) + G36b (frontend pnpm graph). A miss **aborts the release**.
- [ ] **P10.20** [RELEASE] Run the SPDX-expression validation leg + generated-vs-committed NOTICE parity · §3.7.2 §6.3.3 · G36
  needs: P10.18
  > the `spdx` crate / `cargo-about` SPDX-expression validation (poppler `GPL-2.0-only OR GPL-3.0-only`, x265/x264 `-or-later` for the LGPL-3.0 libheif host, libaom `LicenseRef-AOMPL-1.0`) + the parity assertion that every GPL/LGPL/AGPL row has its licence text AND a corresponding-source-POINTER line in `THIRD-PARTY-LICENSES` (the P0.7.1 NOTICE-parity leg).
- [ ] **P10.21** [RELEASE] Assemble + assert the copyleft corresponding-source bundle (imgworker LGPL + x265 GPL) · §6.1.3 §3.6.2 · G38b
  needs: P0.7.2, P10.9
  > execute the P0.7.2 policy at release: ship the static image-worker's complete corresponding source + LGPL object files / relink recipe, **and** the x265 GPL §3 complete corresponding source + written offer (the worker is a GPL combined work when x265 loads); the stage step **fails the build if the source bundle is missing** (G38b). The §5 T6 row's release proof.
- [ ] **P10.22** [RELEASE] Emit the SBOM-diff-between-releases informational asset · §6.3.1 · G35b
  needs: P10.14
  > diff this release's CycloneDX against the previous, surfacing added/removed/changed components as a non-blocking Co-Pilot review item (G35b) — a careless P5–P7 transitive `.so` entering the bundle is otherwise unreviewed.

---

### Auditable binary & bundled-engine CVE awareness

- [ ] **P10.23** [RELEASE] Build the shipped Rust core with `cargo auditable build --release` + assert SBOM↔embedded-list agreement · §3.7.2 · G55 G35
  needs: P0.7.9, P10.9
  > `cargo auditable build --release` embeds the dependency list in the binary (the offline "audit-it-yourself" half); a release-tier sub-assertion extracts it (`cargo audit bin`) and asserts the Rust-component set **==** the CycloneDX SBOM's Rust components, and runs `cargo audit bin`/grype against the shipped binary — the two halves proven to agree (P0.7.9).
- [ ] **P10.24** [RELEASE] Run the informational PURL-keyed OSV/grype scan over `engines.lock` + emit the dated open-CVE release asset · §3.4.3 §6.5 · G17b
  needs: P0.7.7, P10.14
  > the §6.3.4/§6.5 informational scan over the **PURL-keyed** `engines.lock` (a bare `(name,version)` matches nothing — the FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>` MANDATORY + poppler/libheif/libde265/libvips/LibreOffice CPEs); emit the **dated** open-CVE report (recording advisory-DB age) as an owner-signed-off release asset (G17b). Non-blocking per-push.
- [ ] **P10.25** [RELEASE] Wire the CVSS≥7-on-an-exercised-path release-blocking escalation + the advisory-DB staleness floor · §3.4.3 §6.5 · G17b G17
  needs: P10.24, P0.6.9
  > the §6.3.4 rule: a **CVSS ≥ 7 on an actively-exercised §04-format path → release-blocking escalation** (vuln-response.md / `SECURITY.md`), turning the best-effort-currency posture into a real release detector; plus the release-tier **advisory-DB staleness floor** (`cargo audit --json .database.last-updated` + the OSV/grype DB timestamp vs a committed `MAX_ADVISORY_DB_STALENESS` ≈ 7 days, offline-tolerant; shared with G17).

---

### Checksums, minisign signature & verify-recipe (Lane-B stage 6)

- [ ] **P10.26** [RELEASE] Compute per-asset SHA-256 + assemble `SHA256SUMS` over every release asset · §6.2.3 · G39
  needs: P10.12, P10.11, P10.14, P10.18
  > compute a SHA-256 immediately after each artifact is built (before upload), publish a per-file `<artifact>.sha256` sidecar **and** a single `SHA256SUMS` manifest covering **every** asset — the platform binaries AND the SBOM (CycloneDX/SPDX), `NOTICE`/`THIRD-PARTY-LICENSES.txt`, `reliability-report.json`, the measured-sizes line, the CVE report, etc. (`SHA256SUMS` is the only asset it cannot list — the minisig covers it). G39 first half.
- [ ] **P10.27** [RELEASE] Run the `minisign -Sm SHA256SUMS` sign step on the host-isolated ephemeral runner · §6.2.3 §6.7.2 · G39 G56
  needs: P10.26, P10.6, P10.7, P0.7.5
  > the **only** signing in scope: `minisign -Sm SHA256SUMS` producing the detached `.minisig`, on the `environment: release` human-approved, host-isolated ephemeral GitHub-hosted runner (P10.6/P10.7). The private key is `MINISIGN_SECRET_KEY` (passphrase `MINISIGN_PASSWORD`), never committed, never in the bundle. NOT binary code-signing/notarization (SSOT Out of Scope; the former G40 is deleted). → executes the P0.7.5 minisign-over-`SHA256SUMS` signing policy (`needs: P0.7.5`, the signing-policy home, `[x]` before the loop — the one missing link in P10's otherwise-complete P0.7 back-reference set).
- [ ] **P10.28** [RELEASE] Run the release-tier verify-recipe assertion — the LITERAL `minisign -Vm SHA256SUMS -p docs/minisign.pub` · §6.2.3 §6.2.4 · G39 G44
  needs: P10.27
  > a release-tier gate **runs the literal** `minisign -Vm SHA256SUMS -p docs/minisign.pub` (lowercase `-p` = pubkey FILE PATH; `-P` is the inline-base64 flag and would FAIL on a path) against the just-produced `SHA256SUMS` + `.minisig` + committed pubkey, **failing the release on non-zero** — so "recipe present" becomes "recipe correct and working" (G39/G44).
- [ ] **P10.29** [RELEASE] Assert the committed `docs/minisign.pub` matches its out-of-band fingerprint anchor · §6.2.3 · G39 G44
  needs: P10.28
  > a sub-assertion that `docs/minisign.pub` matches a pinned out-of-band fingerprint anchor (a README via the verified GitHub web UI the pipeline can't rewrite) — so a pipeline that could rewrite the pubkey cannot silently substitute a key. Closes the "attacker replaces both artifact AND its key" gap (P0.7.5/P0.7.16).
- [ ] **P10.30** [RELEASE] Enforce the minisign key genesis/backup/loss-recovery custody at the pipeline · §6.2.3 · G39
  needs: P0.7.16, P10.27
  > wire the P0.7.16 custody policy into the release: the keypair was generated air-gapped off the shared VPS, an offline ENCRYPTED backup of BOTH key + passphrase exists off-platform (a deleted single GitHub-secret copy = permanent inability to sign continuations), and the loss-recovery decision path (restore-and-continue vs rotate). The release notes carry the announced rotation trail (retired-key commit + `docs/minisign-retired.pub`) on a re-key.

---

### Build-provenance attestation & reproducibility evidence

- [ ] **P10.31** [RELEASE] Generate `actions/attest-build-provenance` on the release job (scoped `id-token: write`) · §6.7.2 · G59 G58
  needs: P0.7.6, P10.27
  > the v1 OWNER-DECIDED build-ORIGIN signal additive to minisign (binds artifact↔runner+workflow+commit so a re-signed release from a poisoned host is detectable even if the key leaked); needs `id-token: write` scoped to **ONLY** the attestation step. Recorded DECIDED in `gate-status.md` (P0.7.6).
- [ ] **P10.32** [RELEASE] Verify the attestation on a clean runner (`gh attestation verify`) — fail on non-zero · §6.7.2 · G59
  needs: P10.31
  > VERIFIED, not just generated: a release-tier step runs `gh attestation verify` against the just-produced artifact on a clean runner, **failing on non-zero** (P0.7.6).
- [ ] **P10.33** [RELEASE] Publish the Sigstore bundle + paired `trusted_root.jsonl` as offline-verifiable release assets · §6.7.2 · G59 G58
  needs: P10.31
  > name the Sigstore bundle + a paired `trusted_root.jsonl` (`gh attestation trusted-root`) as release assets so users verify OFFLINE: `gh attestation verify <artifact> --bundle <file> --custom-trusted-root trusted_root.jsonl --repo Ne-IA/convertia`. Added to the G58 enumeration.
- [ ] **P10.34** [RELEASE] Emit the `diffoscope` third-party-reproducibility delta of the self-compiled layer (informational) · §6.2.5 · G60
  needs: P0.7.10, P10.9
  > the `diffoscope` delta of the self-compiled Rust-core + WebView layer (NOT the vendored engines) as an **informational** release asset — best-effort, explicitly **NOT** a release gate (vendored-engine non-determinism cannot fail it); a Co-Pilot review item (P0.7.10). G59 proves ORIGIN, G60 proves DETERMINISM of the bytes we own.
- [ ] **P10.35** [RELEASE] Finalize `docs/reproduce.md` + the build-environment lock (the human rebuild recipe, incl. the from-source engine-build recipe) · §6.2.5 §3.8 · G60
  needs: P0.7.10, P10.8, P4.28.1
  > fill the P0.7.10 `docs/reproduce.md` rebuild recipe + build-environment lock (pinned base-image digest, `rust-toolchain.toml`, Tauri CLI/bundler digest, exact build command, expected per-file SHA-256 of the Rust core) an independent party can follow; apply the cheap determinism measures (`SOURCE_DATE_EPOCH` where honoured, recorded toolchain/engine versions, §6.2.5). **From-source engine-build scope extension (provenance symmetry, since the from-source curated engine builds now exist — P4.28.1 + the per-engine compiles P5.1.1/P5.5.1/P5.9.1/P6.1.1/P7.17.1):** ALSO document the per-engine from-source build recipe + the **digest-pinned build container** (the P4.28.1 harness's pinned base-image digest + per-engine `<engine>.configure.flags` manifest), so an independent party following reproduce.md can rebuild the SHIPPED engine binaries (the highest-value attack surface), not only the bytes-we-own Rust core. Kept **best-effort / non-gating** per §6.2.5 (vendored-engine non-determinism cannot fail the gate) but the engine-build recipe is a NAMED artifact, not silently out of scope. (`needs: P4.28.1` — the from-source compilation harness whose container + configure-flag manifest the recipe names.)

---

### Artifact-size gate & pre-publish archive validity (Lane-B stage 1)

- [ ] **P10.36** [RELEASE] Build the ≤400 MB compressed artifact-size gate — measure per platform, fail on exceed, publish sizes · §6.1.2 §3.9.2 §6.7.2 · G41
  needs: P0.7.11, P10.12, P10.11
  > immediately after stage-1 build, measure each platform's **compressed** artifact and **fail the release if any exceeds the §3.9.2 ≤400 MB ceiling** (DoD row 22); record the measured sizes as a release-asset line (the size *levers* are owned P4; the *gate* is here). Activates G41 / the P0.7.11 budget policy.
- [ ] **P10.37** [RELEASE] Run the pre-publish archive-validity leg — each artifact is an OPENABLE archive · §3.5.4 · G41b
  needs: P10.12, P10.11
  > before publishing, a <30 s leg asserts each artifact is a valid openable archive (not just size-checked): `unzip -t` (Windows `.zip`), `hdiutil verify` (macOS `.dmg`), `--appimage-extract-and-run`/`file`+`sha256sum` (Linux AppImage) — a corrupt artifact passing the size check is otherwise found only by users (G41b, P0.7.4).

---

### No-system-pollution gate (Lane-B stage 1, §6.10 row 21)

- [ ] **P10.38** [GATE] Build the no-system-pollution before/after STATE snapshot-diff (the load-bearing CLI-automatable leg) · §6.10 §7.4 §7.8.2 · G43
  needs: P0.7.12, P10.9
  > the §6.10 row-21 load-bearing leg: run the built app under a conversion and assert via a **before/after state snapshot-diff** (CLI-automatable on every OS) that there are **no registry writes** (Win — `reg export` of HKCU+HKLM\SOFTWARE + FS diff), **no `LaunchAgent`/`LaunchDaemon` install** (macOS — `LaunchAgents`+`LaunchDaemons`+file-association DB enumeration + `lsof +D`, NOT the SIP-blocked `fs_usage` live trace), **no system-service/unit install** (Linux — `~/.local`+`~/.config`+desktop-dir diff), **no file-association registration** (§7.8.2), and **no writes outside** the OS config/log dir + the user's chosen output. A pollution write fails the gate (G43). Activates P0.7.12's G43 leg.
- [ ] **P10.39** [GATE] Wire the live syscall/fs monitor as the per-OS best-effort leg (Linux authoritative) · §6.10 · G43 G24
  needs: P10.38
  > the live-monitor half: `strace`+inotify (Linux — the **authoritative** live leg) / Procmon (Windows, informational-where-available) / a config-dir watch (macOS, informational) during the conversion, complementing the snapshot-diff; ships a G24 positive+negative self-test (a planted registry/LaunchAgent/association write MUST fail it).

---

### Governance-completeness gate (Lane-B governance assertion)

- [ ] **P10.40** [DOC] Verify/finalize the five governance docs authored in P1.42–P1.46 are non-stub for release (no re-authoring) · §6.8 · G44
  needs: P1.42, P1.43, P1.44, P1.45, P1.46
  > **Boundary (each fact one home, `_format.md` §8):** the five governance docs are **AUTHORED in P1** (`CONTRIBUTING.md` P1.42, `CODE_OF_CONDUCT.md` P1.43, `SECURITY.md` P1.44, `PRIVACY.md` P1.45, `TRADEMARK.md` P1.46) — they gate contribution from the first commit and have no build dependency. **P10.40 does NOT re-author them** (that would be a duplicate-home violation + a DoD-(b) no-op). It is the release-time finalize step: confirm each P1-authored doc is **non-stub / release-ready** — `CONTRIBUTING.md` carries inbound=outbound MIT + no-CLA + optional-DCO + inbound-warranty + the quality-bar list stated directly (not by reference to the private `CLAUDE.md`); `SECURITY.md` carries the §0.11 untrusted-decoder scope (back-filled by P4's threat-map assembly) + §2.12 isolation + no-SLA + §7.5-redacted repro; `PRIVACY.md` the §2.11 offline/no-telemetry + cloud-sync caveat; `TRADEMARK.md` the name/logo carve-out; `CODE_OF_CONDUCT.md` the enforcement contact — applying any release-line content fix in place. The machine-checkable completeness GATE is P10.41. (`LICENSE`/`NOTICE`/`THIRD-PARTY-LICENSES` are the P1/P10.18 set.)
- [ ] **P10.41** [GATE] Build the governance-doc completeness release gate — present + non-stub + key-section grep · §6.8 · G44
  needs: P10.40, P0.7.13
  > the §6.8 Lane-B assertion that all **five** governance docs are present AND non-empty (a ≥200-byte floor defeating empty placeholders) **plus** a `grep` for one required key section per file (`SECURITY.md`→private-advisory/report heading; `PRIVACY.md`→offline/no-telemetry; `TRADEMARK.md`→name/logo carve-out; `CONTRIBUTING.md`→inbound=outbound + quality-bar list; `CODE_OF_CONDUCT.md`→enforcement contact). A missing/stub file fails the Lane-B gate (G44). Checks existence + non-emptiness + grep, NOT prose quality.
- [ ] **P10.42** [GATE] Wire the `docs/demoted-pairs.md` ↔ pair-status-ledger consistency leg into the governance gate · §6.5.3 §6.8 · G44
  needs: P10.41
  > the same §6.8 gate asserts every §6.5.2 ledger entry in state `unavailable-per-§3.4` or `demoted` has a matching `docs/demoted-pairs.md` row (required fields: pair, kind, affected platforms, reason, ledger ref) **and vice-versa** (no orphan rows) — so a patent-gapped/demoted pair can never ship without its release-note item, making §6.10 rows 16/17 a concrete machine-checkable gate (G44).
- [ ] **P10.43** [DOC] Wire the `.github/` policy set + the SSOT-default-to-Parked issue templates · §6.8 · G44
  needs: P10.40
  > the `.github/` issue templates (new-format/feature requests default to **Future Ideas (Parked)** per the SSOT inclusion test), the PR template referencing the DCO/quality bar, and the private-advisory config wired to `SECURITY.md`; the DCO `Signed-off-by` is **requested, not required** (CI does not hard-block an unsigned commit — that would make it required — but may surface a friendly reminder).

---

### Name/trademark clearance gate + rename propagation

- [ ] **P10.44** [DOC] Finalize `docs/name-clearance.md` — verdict = clear, dated for the release line · §6.9.1 · G45
  > the owner clearance record for **both** "ConvertIA" and the public "Ne-IA" brand: marks checked, jurisdictions/registries searched (EU/EUIPO + US/USPTO + app-distribution-region sanity + crates.io/npm/GitHub-org/app-listing collision search), date, findings, verdict = **clear** (the [DECIDED] v1 verdict). *Registering* a mark stays out of scope; the in-repo clearance check is in scope.
- [ ] **P10.45** [GATE] Build the clearance-record release gate — present + current + verdict clear · §6.9.2 · G45
  needs: P10.44, P0.7.13
  > Lane-B stage 4: assert `docs/name-clearance.md` exists, is dated for the current release line, and its verdict is `clear` (or `conflict→rename` with a completed rename); a `conflict→abort` or a missing/stale record **blocks the release** (G45). CI checks the record; the human does the check.
- [ ] **P10.46** [RELEASE] Build the dormant scripted rename-propagation pass (`scripts/rename-brand.*`) · §6.9.3 · G45
  > the dormant `scripts/rename-brand.*` that, on a `conflict→rename`, propagates a new name across **every** surface before release: `Cargo.toml` (crate + `productName`), `package.json`, `tauri.conf.json` (`productName`/`identifier`/window title/bundle name), repo/org refs, `LICENSE`/`NOTICE`/`TRADEMARK.md` name lines, README + all governance docs + download page + verify recipe, the logo/icon/About strings + `.desktop`/Info.plist names, the in-app product strings, the SBOM/`engines.lock` product field. A documented capability, dormant for v1 (verdict clear).
- [ ] **P10.47** [GATE] Build the post-rename old-name grep gate over repo + staged bundle · §6.9.3 · G45 G24
  needs: P10.46
  > a CI grep asserting the old name appears **nowhere** in shippable artifacts (an `rg` over the repo + the staged bundle for the old token, excluding historical changelog entries); ships a G24 self-test (a planted old-name token MUST fail it). Runs only when a rename was applied; for v1 (verdict clear) the rename machinery stays dormant.

---

### Download / trust-page content (§6.2.4)

- [ ] **P10.48** [DOC] Author the README download/trust page — canonical location + as-is/no-warranty + supported-OS floor · §6.2.2 §6.2.4 · G44
  needs: P10.40
  > the README download/trust section: what ConvertIA is, the **canonical-GitHub-Releases-only** download location (`github.com/Ne-IA/convertia/releases` — no mirror/third-party host endorsed, §6.2.2), the as-is / no-warranty / best-effort-security posture (SSOT License & Openness), and the §0.3.1 supported-OS floor (referenced, not re-decided).
- [ ] **P10.49** [DOC] Author the copy-paste verify-hash recipe incl. the literal minisign verify line · §6.2.4 · G44
  needs: P10.48
  > the verify recipe at the highest-risk moment: Windows `Get-FileHash .\ConvertIA-<version>-x64.zip -Algorithm SHA256` (hash the portable `.zip`, not a loose `.exe`); macOS/Linux `shasum -a 256 ConvertIA.dmg` / `sha256sum ConvertIA.AppImage` or `sha256sum -c SHA256SUMS`; and the literal `minisign -Vm SHA256SUMS -p docs/minisign.pub` (lowercase `-p`). The G44 literal-form recipe assertion (P10.28) re-checks this is the working form.
- [ ] **P10.50** [DOC] Author the macOS Sequoia step-by-step Gatekeeper + per-sidecar quarantine recovery · §6.2.4 · G44
  needs: P10.48
  > the step-by-step unsigned-build first-launch flow (the Control-click bypass is gone on Sequoia): double-click → "can't be opened" → System Settings → Privacy & Security → "Open Anyway" → **on the final confirm dialog click "Open"** → re-launch; **plus** the per-sidecar note that each bundled tool is independently quarantined so the **first conversion** may need the same Privacy-&-Security step per sidecar (surfaced in-app as `QuarantinedByOs`, §2.8/§7.2.4). The §6.6 macOS walkthrough (P11) validates this copy gets a non-technical user through.
- [ ] **P10.51** [DOC] Author the Windows SmartScreen + WebView2 prerequisite note · §6.2.4 §0.3.1 · G44
  needs: P10.48
  > the Windows SmartScreen friction ("Windows protected your PC" → More info → Run anyway) **and** the WebView2 prerequisite note — because the portable `.zip` cannot show an in-app fault when WebView2 is absent (the loader fails before the core runs), this note is the **sole Windows floor mechanism in v1** (no NSIS bootstrapper): *"ConvertIA needs Microsoft Edge WebView2 (built into Windows 11 and current Windows 10; if a window flashes and closes, install the WebView2 Runtime or update Windows/Edge)."*
- [ ] **P10.52** [DOC] Author the Linux AppImage libfuse2 prerequisite note · §6.2.4 §6.1.4 · G44
  needs: P10.48
  > the FUSE-2-at-launch runtime-dependency disclosure: *"Linux: the AppImage needs `libfuse2` (Ubuntu: `sudo apt install libfuse2`, or `libfuse2t64` on 24.04+); alternatively run with `--appimage-extract-and-run`."* — a bare "download, run, done" is false on a FUSE-3-only distro.
- [ ] **P10.53** [GATE] Build the download/trust-page completeness assertion (parse-checked prerequisite notes) · §6.2.4 §6.8 · G44
  needs: P10.49, P10.50, P10.51, P10.52, P0.7.13
  > the G44 leg asserting the download/trust page is complete: the **literal-form** minisign-recipe assertion (P10.28) + the **parse-checked** WebView2/libfuse2/macOS-Sequoia prerequisite notes are present (P0.7.13's "literal-form minisign-recipe + parse-checked prerequisite notes"). A typo'd/absent note is a trust-damaging defect, so this is gated, not trusted.

---

### Release-artifact completeness meta-gate, usability-evidence gate & publish (Lane-B stages 5 & 7)

- [ ] **P10.54** [RELEASE] Build the release-artifact completeness meta-gate (G58) — enumerate every required asset, fail if any missing · §6.8 §6.9 · G58
  needs: P10.26, P10.21, P10.33, P10.24, P10.36
  > the single backstop catching "an asset silently didn't get attached": enumerate **EVERY** required release asset — per-OS bundle, `SHA256SUMS` + `.minisig`, `.sha256` sidecars, SBOM (CycloneDX + SPDX), dated open-CVE report, `NOTICE`/`THIRD-PARTY-LICENSES.txt`, the copyleft corresponding-source bundle, the measured-sizes asset, `reliability-report.json`, `docs/usability-floor.md`, `docs/name-clearance.md`, the §6.5.3 CHANGELOG/release-notes, the G59 Sigstore bundle + `trusted_root.jsonl` — fail if any is missing (P0.7.13).
- [ ] **P10.55** [GATE] Assert every enumerated asset has a corresponding line in the signed `SHA256SUMS` · §6.2.3 §6.8 · G58 G39
  needs: P10.54, P10.26
  > the G58 second predicate: every enumerated release asset (P10.54) has a matching line in the signed `SHA256SUMS` (so a published-but-unhashed asset is caught) — the meta-gate ties asset-presence to integrity coverage.
- [ ] **P10.56** [RELEASE] Build the usability-floor evidence gate (Lane-B stage 5) + the staleness criterion · §6.6 · G44
  needs: P0.7.11, P10.4
  > Lane-B stage 5 asserts `docs/usability-floor.md` records passing walkthroughs for all three platforms for **this** release line (the *evidence* CI checks; P11 performs the walkthroughs that produce it). Wire the machine-checkable **staleness criterion**: each record carries a `release_line` + `date`; the gate **fails** if the `release_line` does not match the release being built (or, absent a version match, if `date` predates the git tag's commit date, `date >= git log -1 --format=%ai <tag>`) — an old walkthrough cannot silently satisfy a new release.
- [ ] **P10.57** [RELEASE] Author the §6.5.3 CHANGELOG / release-notes incl. the two-exception release-note items · §6.5.3 · G58
  needs: P10.42
  > the release `CHANGELOG.md`/GitHub Release body: the human-readable projection of the §6.5.2 ledger's `unavailable-per-§3.4` (exception 1, patent gap) + `demoted` (exception 2) rows mirrored from `docs/demoted-pairs.md`, plus the as-is/no-warranty restatement + the verify-hash recipe — so no patent-gapped/demoted pair ships as a silent omission. Enumerated in G58 (P10.54).
- [ ] **P10.58** [RELEASE] Wire the Lane-B publish stage to canonical GitHub Releases — one coordinated all-or-nothing release · §6.2.2 §6.7.2 · G58
  needs: P10.55, P10.27, P10.32, P10.19, P10.45, P10.56, P10.37
  > Lane-B stage 7: upload artifacts + `SHA256SUMS` + `.minisig` + `.sha256` files + SBOM (CycloneDX/SPDX) + `reliability-report.json` + `NOTICE`/`THIRD-PARTY-LICENSES.txt` + the attestation bundle as a **single coordinated release** (one large all-or-nothing v1, SSOT); the release body restates as-is/no-warranty + the verify recipe (P10.49) + the two-exception items (P10.57). No auto-update/phone-home publishing step (P10.1/P10.2). Runs only after every prior release-blocking stage is green.

---

### First-party crate-trust vetting (the deferred cargo-vet live run)

- [ ] **P10.59** [GATE] Run the deferred cargo-vet first-party crate-trust live vetting — `init`/`import` (≥2 DBs) / `check --locked` over the full `Cargo.lock`, commit `imports.lock`, populate `audits.toml` · §3.8 · G18b G18a G9
  needs: P1.7
  > the live cargo-vet run that P0.3.6 authored the config/protocol for and **P1.59 deferred** ("a separate P10 vetting effort"): pin cargo-vet (`gate-tools.toml`), `cargo vet init`, `cargo vet import` the ≥2 declared DBs (Mozilla + Google, `supply-chain/config.toml`), `cargo vet suggest` then certify/exempt the full tree (~471 crates) with documented reasons, commit `supply-chain/imports.lock` (Co-Pilot-reviewed, never auto-fetched — G9 invariant (e) / G18a `--locked`), and populate `supply-chain/audits.toml`. This flips the G18b live tier from skip-with-warning to fail-closed (a clean `cargo vet check --locked` becomes release-blocking). **Authored as the owning box at P1.68** so the deferred work is plan-owned (the obligation-as-prose lesson the P1.66 G71 fix taught — an unowned, unverified deferral silently never happens). Runs in the P10 supply-chain stage, before P10.58 publish.

---

### The phase-end Co-Pilot hardening sweep — the standing phase-close box

> The standing test-strategy §11 phase-close box (owner directive, recorded 2026-07-06):
> Co-Pilot-executed — never the Build-Loop; mandate, level and evidence rules in
> [test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep).

- [!extern] **P10.60** [TEST] Run the phase-end Co-Pilot hardening sweep over the whole P10 delivery — adversarial re-test at the hardest technically-possible level · §6.4
  > **[!extern] (Co-Pilot-executed — the standing test-strategy §11 phase-close sweep, never the Build-Loop):** runs once every other P10 box is `[x]`; the phase's whole delivery is adversarially re-tested at the hardest technically-possible level with unrestricted session tooling (Docker, WebDriver/Playwright, property/fuzz/mutation probes, real-OS live runs); findings are fixed with tests as normal dual-reviewed commits before this box flips `[x]`.
  > **Boundary stop:** P11.1 carries `needs:` on this box — a `[!extern]` prerequisite of a non-extern box is a loop STOP (`_format.md` §2/§6), so the loop hard-stops at the P10→P11 boundary and hands off to the Co-Pilot until the sweep is `[x]`.
