# P11 — Final E2E & Acceptance

> **The release candidate is verified end-to-end and signed off.** P11 is a
> **pure VERIFICATION phase**: the gate machinery it exercises was *built* in
> P2/P4–P10 — P11 only **proves it green on the RC** and records the sign-off. It
> contributes **no new gate, no new mechanism, no new product behaviour**; every box
> below runs an already-built gate against the release candidate, collects its
> evidence, and either passes or sends the RC back for a fix. The phase ends when the
> full SSOT *v1 Definition of Done* is demonstrably satisfied and the RC is signed
> off.
>
> **Spec home:** [06-build-test-release](../spec/06-build-test-release.md)
> (§6.5 reliability gate, §6.6 usability-floor walkthrough, §6.4.6 headed-E2E,
> §6.10 DoD-traceability checklist), [07-app-shell §7.2.3](../spec/07-app-shell.md)
> (startup-integrity & engine-presence gate, §7.2.4 quarantine/exec-permission,
> §7.2.5 orphan reclaim, §7.1.1 single-instance),
> [SINGLE-SOURCE-OF-TRUTH.md §9](../SINGLE-SOURCE-OF-TRUTH.md) (v1 Definition of
> Done). Index: [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **Reads / proves, never builds:** the §6.5 reliability gate + pair-status ledger
> (built P4, populated P5–P7), the §6.4.6 headed-E2E harness (`tauri-driver` +
> WebdriverIO, built P9), the §7.2.3 startup verifier (built P4), the offline-egress
> observability gate G42/G42b (built P9), the no-system-pollution gate G43 + the
> ≤400 MB size gate G41 + the governance/clearance gates G44/G45/G58 (built P10).
> P11 wires nothing into the gate framework — it asserts each is green on the RC and
> binds the human-walkthrough evidence to the release line. Because P11 is the
> highest phase, the loop's lowest-phase-first scan guarantees P1–P10 are complete
> before any P11 box is selected; cross-phase prerequisites built in other phases are
> therefore named in prose `>`-notes (the phase that owns them), and `needs:` is used
> only for **intra-P11 ordering** (a P11 box that must consume another P11 box's
> output first).
>
> **The six §6.6 usability-floor sub-gates are SEPARATELY FAILEABLE.** Per the
> README P11 scope and §6.6, the usability floor is not one box: it is **six
> distinct, independently-faileable boxes** — (a) the non-developer conversion
> walkthrough, (b) the keyboard-only pass, (c) the screen-reader smoke pass, (d) the
> mandatory macOS Sequoia first-launch + per-sidecar quarantine recovery sub-test,
> (e) the `docs/usability-floor.md` artifact + its machine-checkable staleness
> criterion, (f) the single-instance double-extract macOS sub-test — so a failure in
> any one is attributable and re-walked on its own.
>
> **This is the v0 BASE** — the smallest-atomic `[ ]` boxes are below, grouped under
> `### ` sub-headings; a later adversarial review will deepen, split and complete
> them.

---

### RC assembly & freeze (the artifact under test)

> P11 verifies *a specific release candidate*. These boxes establish the exact bytes
> every later box runs against, so the reliability run, the egress run, the size
> measure, the human walkthrough, and the sign-off all point at the **same** RC and
> the **same** git tag — never a drifting working tree.

- [ ] **P11.1** [RELEASE] Cut the RC tag and trigger the Lane-B release pipeline against it · §6.7.2 · G56b
  > create the signed annotated `v*` RC tag on a green `main`, push it, and confirm the §6.7.2 Lane-B release workflow starts against that exact SHA (the G56b ancestry/green-history first-step abort proves the tag is an ancestor of `origin/main` with main's required checks green). This box defines the single RC SHA every P11 verification box asserts against. → the Lane-B pipeline itself is built in P10; this box only triggers it.
- [ ] **P11.2** [RELEASE] Collect the per-platform RC artifacts + `reliability-report.json` as the verification inputs · §6.1.2 §6.5.2 · G30 G31
  needs: P11.1
  > gather the three Lane-B build outputs (Windows portable `.zip`, universal `.dmg`, AppImage) and the generated `reliability-report.json` produced by the RC run, recording each artifact's SHA-256 so every P11 box that "runs the built app" provably runs the RC bytes (not a local rebuild). The build matrix + ledger generator are P4/P10's; this box freezes their RC outputs.
- [ ] **P11.3** [DOC] Record the RC acceptance manifest (tag, SHAs, evidence index) as the sign-off anchor · §6.10
  needs: P11.2
  > stand up the tracked acceptance manifest (the RC tag, the per-platform artifact SHAs from P11.2, and an index pointing at each evidence artifact — the ledger, the egress-run log, the size-asset line, `docs/usability-floor.md`) so the final RC sign-off (P11.7) checks off against one auditable record rather than scattered run logs. The §6.10 ref homes the DoD-traceability the manifest mirrors.

---

### Cross-platform E2E test matrix (the automated flow gate)

> The README P11 scope's "cross-platform E2E test matrix": prove the §6.4.6 headed
> E2E flow runs green on every platform leg of the RC. The harness is built in P9;
> P11 confirms it passes against the frozen RC, and proves the macOS degraded-smoke
> leg is genuinely exercised (not silently skipped).

- [ ] **P11.4** [TEST] Verify the Windows headed-E2E flow passes on the RC (tauri-driver + WebdriverIO) · §6.4.6 · G33a
  needs: P11.2
  > run the §6.4.6 WebdriverIO/`tauri-driver` flow (empty → file-picker intake → collected/confirm → target+default → destination-shown → progress → summary → open-folder) against the RC Windows artifact and assert it passes, including the Idle "all conversion happens locally, on your machine" reassurance-line presence check. The harness (`wdio.conf.js`, the `msedgedriver`↔WebView2 match) is built in P9; this box asserts the RC leg green.
- [ ] **P11.5** [TEST] Verify the Linux headed-E2E flow passes on the RC under Xvfb (extracted AppImage ELF) · §6.4.6 · G33a
  needs: P11.2
  > run the §6.4.6 flow against the RC AppImage on the Linux leg — `--appimage-extract`, glob `squashfs-root/usr/bin/*` for the `productName`-cased binary, point `tauri:options.application` at the extracted ELF, wrap in `xvfb-run`, then `rm -rf squashfs-root/` — and assert the full flow passes. The Xvfb/extract wiring is built in P9; this box asserts the RC leg green.
- [ ] **P11.6** [TEST] Verify the macOS degraded-smoke leg passes on the RC (launch + synthetic-argv conversion + exit 0) · §6.4.6 · G30
  needs: P11.2
  > run the §6.4.6 macOS degraded smoke (no WKWebView WebDriver exists): launch the RC `.app`, drive a synthetic `argv` conversion of one corpus file through the §7.8 launch-intake path in a `TMPDIR` (no TCC prompt), and assert (a) window/process present, (b) expected output file appears, (c) exit 0. Confirms macOS DoD-#7 satisfaction at the automatable level; the WebView-UX half is the §6.6 human walkthrough (P11.3).
- [ ] **P11.7** [TEST] Assert the E2E matrix is COMPLETE — every required platform leg ran and reported · §6.4.6 §6.4.4 · G30
  needs: P11.4, P11.5, P11.6
  > a completeness check over the matrix run that fails if any of the three platform legs (Windows full-WebDriver, Linux full-WebDriver-under-Xvfb, macOS degraded-smoke) is **absent or skipped** — so a silently-dropped leg cannot pass the E2E gate by omission. The per-platform run mechanics are §6.4.4/§6.4.6; this box owns the "all legs present" assertion for the RC.

---

### §6.6 usability-floor walkthrough (six separately-faileable sub-gates)

> The SSOT *v1 DoD* usability floor (§6.6): a human who did not build ConvertIA
> completes each named conversion unaided. The README mandates this be **six distinct
> atomic boxes**, each independently faileable → fix → re-walk. All six record into
> `docs/usability-floor.md` (the required v1 artifact, P11.12); a failure on any one
> fails the floor for its platform/aspect only.

- [ ] **P11.8** [TEST] Run the non-developer conversion walkthrough — named conversions, two-click common path · §6.6
  needs: P11.2
  > sub-gate (a): a genuine non-developer (did not build/contribute, given no instructions) completes the §6.6 named conversions (`mov→mp4`, `png→webp`, `heic→jpg`, `mp3`-source → its default, `docx→pdf`, `xlsx→csv`, `pptx→pdf`, extract-audio → MP3, clip → GIF) via the two-click path (drop → already-highlighted/pick target → convert) on ≥1 platform, meeting all six §6.6 pass criteria (understands empty screen; sees collected summary; reaches sensible result with the pre-highlighted default; sees destination before converting; uses open-folder/file and finds output; hits no stack trace/cryptic message/dead end). A stuck/needs-help task fails the floor for that platform → fix → re-walk. Also validates the §04 genuinely-debatable per-source defaults (XLSX→CSV vs →PDF; MP3-source→WAV vs FLAC). Owner may run the remaining two platform walkthroughs (amended SSOT §9 / §6.6 tester-sourcing).
- [ ] **P11.9** [TEST] Run the keyboard-only pass over the core path · §6.6 §5.10
  needs: P11.2
  > sub-gate (b): at least one walkthrough completes the core path **keyboard-only** per the §5.10 shortcut map (drag/drop+picker+keyboard reach the same result), and verifies readable contrast/text-size by eye — the human half of the DoD basic-a11y gate, complementing the automated axe-core assertions (§6.4.6a / P11.20). Recorded in `docs/usability-floor.md`.
- [ ] **P11.10** [TEST] Run the screen-reader smoke pass over Idle→Collecting→Confirm→Converting→Summary · §6.6 §5.6.1
  needs: P11.2
  > sub-gate (c): one screen-reader walkthrough on ≥1 platform with the native SR (VoiceOver/NVDA/Orca) steps the §5.6.1(3) per-state traversal table and confirms, against §5.6.1(1)/(2): every state has a reachable non-orphaned landing element; the collected summary + confirm-gate string announce assertively; progress milestones announce (not every tick); decision dialogs announce as `alertdialog` with their accessible name; lossy/divert notes announce politely. This is the verification gate for the §5.6.1 implementable SR contract — axe-core (P11.20) proves ARIA validity, not usable announcement. Records which SR + which platform in `docs/usability-floor.md`; feeds §6.10 DoD row 6.
- [ ] **P11.11** [TEST] Run the mandatory macOS Sequoia first-launch + per-sidecar quarantine recovery sub-test · §6.6 §7.2.4 · G46
  needs: P11.2
  > sub-gate (d): the macOS walkthrough runs on **Sequoia (15.x)** from a freshly **downloaded (quarantined)** RC artifact and the tester succeeds at **both** (1) blocked-app first-launch recovery (Privacy & Security → "Open Anyway", no Control-click bypass on Sequoia) **and** (2) the first-conversion step where an independently-quarantined sidecar may be blocked — confirming the in-app `QuarantinedByOs` guidance (§2.8/§7.2.4, the message naming the specific blocked sidecar) and the §6.2.4 download-page steps actually get a non-technical user through. A stuck-at-Gatekeeper / silent first-conversion failure fails the macOS floor → revisit unsigned posture / guidance copy → re-walk. Preferentially staffed by a non-dev tester (highest non-technical-user blocker). G46's `QuarantinedByOs`-distinct-kind acceptance is the automated complement (P11.17).
- [ ] **P11.12** [DOC] Author the `docs/usability-floor.md` artifact + assert its machine-checkable staleness criterion against the RC tag · §6.6 · G44
  needs: P11.8, P11.9, P11.10, P11.11, P11.13
  > sub-gate (e): the required-v1 `docs/usability-floor.md` records per platform — tester profile (non-dev vs owner-run), tasks, pass/fail, observed friction, default-validation notes, which SR/platform for the smoke pass — and carries a `release_line` + `date` per walkthrough record. Assert the §6.7.2-stage-5 staleness gate is satisfied for the RC: the recorded `release_line` matches the release being built, or (absent a version match) the recorded `date >= git log -1 --format=%ai <RC tag>` (the tag's commit date) — so an old walkthrough cannot silently satisfy the RC. The Lane-B staleness gate is built in P10; this box produces the artifact and confirms it passes for the RC. → §6.10 DoD row 11.
- [ ] **P11.13** [TEST] Run the single-instance double-extract macOS sub-test · §6.6 §7.1.1
  needs: P11.2
  > sub-gate (f): on macOS, extract the unsigned `.app` **twice** and launch from both copies to confirm the §7.1.1 single-instance / refuse-busy hand-off behaves under `tauri-plugin-single-instance`'s bundle-identity matching — **or** document the limitation if two separately-extracted unsigned copies run as independent instances (an accepted v1 edge per §6.6, recorded in `docs/usability-floor.md`, never a silent gap).
- [ ] **P11.14** [TEST] Assert the §6.6 floor is COMPLETE — all named conversions passed + all platform walkthroughs recorded · §6.6 · G44
  needs: P11.12
  > the §6.6 binding requirement ("≥1 genuine non-dev walkthrough on ≥1 platform; three platform walkthroughs recorded; all named conversions pass before release"): assert `docs/usability-floor.md` records ≥1 true non-dev pass, every named §6.6 conversion passed, and the macOS Sequoia quarantine + double-extract sub-tests are recorded — the floor-level roll-up of sub-gates (a)–(f) that the §6.8 governance-completeness gate (G44, P10) consumes as a present, non-stub artifact.

---

### Definition-of-Done verification against the SSOT

> The README P11 scope: confirm the SSOT *v1 DoD* gates are green on the RC. Each box
> below proves one already-built gate passes against the frozen RC — the §6.5
> reliability gate, the §7.2.3 startup-integrity gate (DoD gate 19), the offline-egress
> gate, the ≤400 MB size gate (row 22), the no-system-pollution gate (row 21), and the
> remaining release-blocking governance/clearance gates. P11 builds none of these; it
> demonstrates each is green.

- [ ] **P11.15** [TEST] Verify the §6.5 reliability gate is GREEN on the RC — every v1 pair `reliable` on all available platforms · §6.5.1 §6.5.2 · G31
  needs: P11.2
  > assert the RC `reliability-report.json` has **every enumerated `(source,target)` pair `reliable` on every platform where it is not `unavailable-per-§3.4` or explicitly `demoted`** — any `failing` cell blocks the release (§6.5.2). Realises SSOT *v1 DoD* conversions clause + §6.10 rows 1/2/14/15. The ledger + per-pair integration tests are built/populated in P4–P7; this box proves the release-gate predicate holds for the RC. → published as a release asset (transparency).
- [ ] **P11.16** [TEST] Verify exceptions 1/2 are recorded — `unavailable-per-§3.4`/`demoted` cells ⇔ `docs/demoted-pairs.md` rows · §6.5.3 · G44
  needs: P11.15
  > assert the §6.5.3 bijection: **every** ledger cell in state `unavailable-per-§3.4` (patent per-platform gap, exception 1) or `demoted` (last-resort reliability demotion, exception 2) has a matching `docs/demoted-pairs.md` row with its required fields (pair, kind, affected platform(s), reason, ledger ref + the §3.4 `available=false` row for exception 1) **and** no orphan rows — a silent omission fails the release. Also assert the §6.5.3 hard precondition that H.264/AAC for the MP4 video default target is recorded if gapped on any platform. → §6.10 rows 16/17; consumed by the §6.8 governance gate (G44, P10).
- [ ] **P11.17** [TEST] Verify §7.2.3 startup integrity & engine-presence — missing/corrupt engine yields an app-fault, not a crash (DoD gate 19) · §7.2.3 §2.13 · G46
  needs: P11.2
  > run the §6.4.2/§6.4.6-headed startup-fault test against the RC: a removed/truncated bundled engine binary yields the plain §2.13 app-fault screen ("A required conversion component is missing or damaged — please re-download…"), **never** a stack trace; the §7.2.3 out-of-band binary-list presence loop + the build-time hash-manifest integrity check + the warm-launch size/magic check fire as specified, populating `EngineHealth`. Includes the `QuarantinedByOs` sub-test (a mocked quarantine spawn-failure yields the **distinct** kind, not `EngineMissing`/`BundleDamaged`, §7.2.3/§7.2.4). The startup verifier + manifest generation are built in P4; G46's runtime verifier is accepted here at release. → §6.10 DoD row 19.
- [ ] **P11.18** [TEST] Verify the offline-egress observability gate is GREEN on the RC — zero outbound + no out-of-input read · §6.7.3 §2.11.4 · G42 G42b
  needs: P11.2
  > assert the §6.7.3 release-confirmation egress run passes on the RC: the §6.4.6 E2E flow runs inside the per-OS egress-DENY window (Linux net-namespace under `unshare --net` composed with `Xvfb`; the macOS/Windows equivalents) with **any outbound attempt failing the test** (G42, incl. the DNS-only zero-DNS sub-assertion + armed-window canary), **and** the symmetric read-half fs-audit proves crafted-input engines cannot read out-of-input files (G42b, T9b), with zero-startup-network (§7.2.2) asserted in the same window. The full per-OS deny window + release-confirmation leg are built in P9; this box proves it green on the RC. → §6.10 DoD rows 4/5.
- [ ] **P11.19** [TEST] Verify the §7.2.2 zero-startup-network assertion holds on the RC · §7.2.2 §2.11 · G42
  needs: P11.18
  > assert the RC adds **zero** network activity at startup (no update check, no license/telemetry beacon, no font/asset fetch — all bundled, CSP forbids remote origins) inside the egress-deny window — the startup-specific leg of the offline invariant, distinct from the conversion-path egress proof in P11.18. The packet-monitor harness is built in P9; this box confirms the RC's startup is silent.
- [ ] **P11.20** [TEST] Verify the automated a11y assertions are GREEN on the RC — ARIA/focus (jsdom) + WCAG-AA contrast (live WebView) · §6.4.6a · G33a G33b
  needs: P11.4, P11.5
  > assert the §6.4.6a automated a11y legs pass on the RC: the Lane-A jsdom `vitest-axe` leg (ARIA-role/state validity + focus-order/roving-tabindex sanity) **and** the Lane-B `@axe-core/webdriverio` WCAG-2.1-AA `color-contrast` session (≥4.5:1 text, ≥3:1 large/UI, **both** themes) on the Linux + Windows legs. The macOS contrast gap is human-covered by the P11.9 readable-contrast check (recorded in `docs/usability-floor.md`), not silently skipped. → the automated half of §6.10 DoD row 6; the human halves are P11.9 (keyboard/contrast/text-size) and P11.10 (SR).
- [ ] **P11.21** [TEST] Verify the ≤400 MB compressed artifact-size gate is GREEN on the RC (row 22) · §6.7.2 §3.9.2 · G41
  needs: P11.2
  > assert the §6.7.2 Lane-B size gate passed for the RC: each platform's **compressed** artifact ≤ the §3.9.2 400 MB ceiling, with the measured sizes published as the release-asset line. The size *levers* are owned in P4 and the *gate* is built in P10; this box confirms the RC measurement is under budget. → §6.10 DoD row 22.
- [ ] **P11.22** [TEST] Verify the §6.10 row 21 no-system-pollution gate is GREEN on the RC · §6.10 · G43
  needs: P11.2
  > assert the §6.10-row-21 / G43 post-launch state-snapshot-diff passed for the RC on every OS: the before/after diff (Windows `reg export` HKCU+HKLM\SOFTWARE + FS diff; macOS LaunchAgents/LaunchDaemons + file-association enumeration + `lsof +D`; Linux `~/.local`/`~/.config`/desktop-dir diff + the authoritative `strace`+inotify live monitor) shows **no** registry/LaunchAgent/daemon/file-association writes and **no** writes outside config+log+chosen-output. The gate is built in P10; this box confirms the RC is pollution-free. → §6.10 DoD row 21 / SSOT Principle 2.
- [ ] **P11.23** [TEST] Verify DoD gate 20 — OS-intake routes through the freeze funnel + no file-association pollution · §7.8.2 §6.10 · G43
  needs: P11.2
  > assert the §6.10-row-20 launch-with-files E2E: an Open-with / launch-args invocation routes through the single §7.8 intake funnel (the UI enters Collecting at startup) **and** the §7.8.2 explicit negatives hold (no file-association registered, no custom URL scheme) — the intake-side complement to P11.22's no-pollution audit. The intake funnel is built in P2 and the negatives are asserted by the P10 pollution gate; this box proves both on the RC.
- [ ] **P11.24** [TEST] Verify DoD gate 18 — single-instance + run-identity (no cross-instance temp clobber) on the RC · §7.1.1 §6.10 · G31
  needs: P11.2
  > assert the §6.10-row-18 single-instance + run-identity behaviour on the RC: a second launch hands off to the running instance (§7.1.1) and per-run/per-instance temp ownership + advisory-lock liveness hold so a second launch cannot clobber the first's scratch or interrupt its freeze. Complements the macOS double-extract human sub-test (P11.13) with the automated property/behaviour assertion built in P2/P3; this box proves it on the RC.
- [ ] **P11.25** [TEST] Verify DoD gate 8 — unwritable/ephemeral-location divert fallback on the RC · §6.10 · G31
  needs: P11.2
  > assert the §6.10-row-8 fallback: the RC's per-location divert + cross-volume strategy works on read-only / USB(FAT/exFAT) / network / temp locations (the §2.7/§2.14 divert path exercised in the corpus run). The `fs_guard` divert primitives are built in P3; this box confirms the RC's divert fallback passes.
- [ ] **P11.26** [TEST] Verify the §6.10 row-7 defaults-registry "no required choices" guard is GREEN on the RC · §1.6 §6.10 · G61
  needs: P11.2
  > assert the §1.6 defaults-registry guard (G61, built in P4.59.2) is GREEN on the RC: the CI-generated consolidated merged `OptionDecl.default` index covers **every** §04-offered `(source,target)` pair and **every** declared option of each carries a `default`, so the SSOT *v1 DoD* "no required choices" promise (drop → default → convert with zero clicks) holds for the RC across ALL categories (images jpeg/webp Q + PNG compression + AVIF speed + ICO size set; audio/video default tables; office) — not merely the per-category default target. Peer of P11.15/P11.20: P11 builds nothing; it proves the gate green. This is the dedicated P11 RC-box for §6.10 row 7's machine-checkable half (the E2E core-UX-flow half of row 7 is P11.4–P11.7 + the §6.6 walkthrough P11.8). → §6.10 DoD row 7.
- [ ] **P11.27** [TEST] Verify the G44 governance-completeness gate is GREEN on the RC (every governance doc present + non-stub + recipe/prereq notes + demoted-pairs bijection) · §6.8 · G44
  needs: P11.16
  > assert the P10-built **G44** governance-completeness meta-gate passed for the RC: every required governance doc present + non-stub, incl. the literal-form minisign verify-recipe + the libfuse2/WebView2/Sequoia prerequisite notes + the §6.5.3 demoted-pairs bijection (from P11.16). Separately faileable from G45/G58 so a governance miss is attributable on its own. → §6.10 DoD rows 9/12.
- [ ] **P11.28** [TEST] Verify the G45 name/trademark clearance-record gate is GREEN on the RC (present + dated + verdict clear + old-name grep clean) · §6.9 · G45
  needs: P11.2
  > assert the P10-built **G45** clearance gate passed for the RC: `docs/name-clearance.md` present, dated for the RC line, verdict = clear; the post-rename old-name grep is clean (or N/A for the v1 clear verdict). Separately faileable from G44/G58. → §6.10 DoD row 10.
- [ ] **P11.29** [TEST] Verify the G58 release-artifact completeness meta-gate is GREEN on the RC (every required asset present + SHA256SUMS-covered) · §6.8 · G58
  needs: P11.21
  > assert the P10-built **G58** release-artifact completeness meta-gate passed for the RC: every required asset present — per-OS bundle, `SHA256SUMS`, `.minisig`, SBOM, dated open-CVE report, NOTICE/THIRD-PARTY-LICENSES, copyleft corresponding-source bundle, measured-sizes asset, `usability-floor.md`, `name-clearance.md`, CHANGELOG/release-notes, the Sigstore bundle — and every enumerated asset has a `SHA256SUMS` line. Separately faileable from G44/G45 so a missing-asset failure is attributable on its own. → §6.10 DoD rows 12/13.

---

### DoD-traceability completeness (every §6.10 row demonstrated)

> The README claim "every behaviour the SSOT promises has a technical home" is itself
> a verifiable gate (§6.10): P11 must show **every** §6.10 row is exercised by an RC
> evidence artifact, with **no** DoD row left unproven. This is the meta-assertion
> over P11.2–P11.4.

- [ ] **P11.30** [TEST] Assert every §6.10 in-scope-gate row maps to a green RC evidence artifact (no unproven DoD row) · §6.10
  needs: P11.26, P11.27, P11.28, P11.29, P11.14, P11.7
  > a completeness check over the §6.10 DoD-traceability table: for each **in-scope-gate** row (1–23), assert a corresponding P11 verification box passed and its evidence is indexed in the P11.3 acceptance manifest — so a DoD row cannot be silently unverified at RC. **Row 7 (Core UX flow) is split across its two verification homes:** the machine-checkable "no required choices" defaults-registry half is **P11.26** (G61); the E2E core-UX-flow half is the cross-platform matrix **P11.4–P11.7** + the §6.6 walkthrough **P11.8** — both must be green for row 7. Confirms the §6.10 "every clause has a §6 mechanism" invariant holds *and was actually run* for this RC (not merely that the table is internally consistent — that is plan-lint/spec-lint's job). **Rows 3 (corpus-exists / §6.4.3a bijection / §6.4.5 minimum-content floor) and 14 (no-harm / atomicity / fail-clearly under crash/cancel/out-of-disk — the §2.1.3 kill-in-the-rename-window family) have NO dedicated P11.x RC-box BY DESIGN:** they are satisfied **transitively-via-green-main** — P11.1's ancestry/green-history abort proves the RC SHA's L4 was green, and G22/G24a (row 3) + the §2.1.3 atomicity/out-of-disk property tests (row 14, the P3.21/P0.5.9 `atomic_publish` kill-fence + the T10 byte-budget case) are fail-closed on every push, so a red row-3/row-14 gate could never have reached green main. This box asserts that green-main provenance for rows 3/14 (P11.1 + the L4 run-log in the manifest) rather than re-running them — the per-row symmetry break is intentional, not a dropped box.
- [ ] **P11.31** [DOC] Assert the explicit non-gates are correctly excluded — visual polish + engine-currency are not RC blockers · §6.10
  needs: P11.30
  > assert the §6.10 explicit non-gate row holds for the RC: subjective visual polish ("modern/eye-candy", P8 scope (ii)) and engine-currency (§3.8 best-effort) are **not** treated as release blockers — so a not-yet-polished visual or a non-latest engine pin does not wrongly block the RC, and conversely no real gate was demoted to "polish" to dodge it. Records the non-blocking determination in the acceptance manifest.

---

### Engine-bump re-validation readiness (the SSOT continuity gate)

> The SSOT requires the full reliability gate to re-run before any bundled-engine
> bump can ship (§6.5.4). At RC time P11 confirms the RC's engine set is the
> validated set and that the re-validation machinery is wired, so a post-RC security
> bump cannot silently regress a pair.

- [ ] **P11.32** [TEST] Confirm the RC engine set matches the validated `engines.lock` + the §6.5.4 re-validation path is wired · §6.5.4 §3.7.2 · G37
  needs: P11.15
  > assert every bundled engine in the RC matches its `engines.lock` `purl`+SHA-256 row (the set the §6.5 ledger was validated against) and that the §6.5.4 full-reliability-gate re-run is wired to fire on any future engine bump (the ledger diff being part of the bump's review) — so the RC is provably the validated engine set and the no-silent-regression continuity gate is live for post-v1. → ties the §6.5 reliability green (P11.15) to the §3.8 vuln-response release path.

---

### RC sign-off

> The terminal box of the entire plan: once every P11 verification box above is `[x]`
> and its evidence is indexed, the RC is signed off. No new behaviour — a recorded,
> auditable acceptance decision.

- [ ] **P11.33** [RELEASE] Sign off the RC — every DoD gate green, every §6.6 sub-gate passed, evidence indexed · §6.10 §6.5 · G58
  needs: P11.31, P11.32
  > the RC sign-off: assert that the P11.3 acceptance manifest shows **all** of — the cross-platform E2E matrix complete + green (P11.7), all six §6.6 usability-floor sub-gates passed (P11.14), every §6.10 in-scope DoD gate verified green on the RC (P11.15–P11.29: reliability §6.5, startup-integrity §7.2.3, offline-egress, ≤400 MB size, no-system-pollution, governance/clearance/artifact-completeness), the DoD-traceability completeness (P11.30) + the non-gate exclusions (P11.31), and the engine-set/continuity check (P11.32) — then record the dated sign-off line against the RC tag. This is the SSOT *v1 DoD* satisfied end-to-end; the release may publish. → the final acceptance box of P1–P11.
