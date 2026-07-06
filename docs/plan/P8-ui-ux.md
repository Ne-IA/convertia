# P8 — UI/UX (full experience + polish)

> **The full designed experience beyond the walking-skeleton UI.** P4 built the
> **generic** UX-correctness primitives once (options-panel shell, lossy-note
> surfacing, progress/cancel, result-actions, the §2.8 error-copy framework, the
> structural-a11y wiring), and P5–P7 registered each format's **option
> DECLARATIONS** against that already-built shell. P8 therefore does **not**
> rebuild those primitives nor re-register per-format declarations — it **completes
> the global chrome + polish**: the ship-gating surfaces UI owns and nothing else
> does (About + NOTICE attribution, Impressum, the About→Releases user-initiated
> link, the settings chrome, the cross-cutting refinement of the §2.8 error /
> edge-state copy and §2.9 lossy presentation that is not a per-format declaration),
> plus the non-blocking visual-polish / Ne-IA branding pass.
>
> **Spec homes:** [05-ui-ux](../spec/05-ui-ux.md) (design system §5.5, drop area
> §5.3/§5.4, format picker §5.3, advanced-options panels §5.3, progress+cancel
> §5.3/§5.8, result actions §5.3/§7.7, empty/error/edge states §5.2/§5.6/§5.7,
> About+NOTICE §5.9, Ne-IA branding §5.5/§5.9, settings §5.5/§7.4),
> [02-guarantees](../spec/02-guarantees.md) (§2.8 fail-clearly catalog, §2.9 lossy
> notes), [07-app-shell](../spec/07-app-shell.md) (§7.6.2 About→Releases link).
> Index: [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **Two scopes, marked per box (README P8):** **(i) ship-gating UI** —
> release-blocking surfaces UI alone owns (About+NOTICE+Impressum, About→Releases,
> settings chrome, cross-cutting error/lossy refinement); a **`P8.<n>` "P8
> ship-gating done"** sub-gate (P8.21) closes scope (i). **(ii) visual-polish /
> branding** — **NON-BLOCKING, may trail the P11 RC** (SSOT §9 marks only
> "modern/eye-candy" polish non-blocking); each scope-(ii) box says so in its note.
>
> **This is the v0 BASE** — the smallest atomic `[ ]` boxes below, derived
> exhaustively from the spec homes; a later adversarial review will deepen, split
> and complete them.

## Boundaries (so P8 does not double-build P1/P4–P7)

- **P1 built** the `src/strings/ui.ts` English-string module shell + the `a11y/`
  module shells (`announcer.ts`, `keymap.ts`). P8 **fills** chrome strings + the
  canonical keymap entries it owns; it does not re-establish the modules.
- **P4 built** the generic options-panel shell, lossy-note surfacing in
  FormatPicker, progress/cancel, result-actions/open-folder flow, the §2.8
  error-copy framework, and the structural-a11y wiring (ARIA roles + keyboard on
  DropZone/FormatPicker/DestinationBar/ProgressList + focus management). P8
  **completes the global chrome** (AppHeader / BrandLogo / ThemeToggle /
  AboutDialog / BusyNotice) and **refines** the cross-cutting copy/states; it does
  not re-implement the primitives.
- **P5–P7 registered** the per-format advanced-option DECLARATIONS (§1.6) against
  the P4 shell. P8 owns **no** per-format declaration.
- **P9 validates** a11y (headed-E2E axe-core contrast G33b, SR smoke, keyboard-path
  equivalence) and **P11** runs the §6.6 usability walkthrough — P8 ships the
  *implementation* those phases verify, not the validation harness.
- **Cross-phase edges carried INLINE (no reconciliation box):** unlike the
  format-exercise phases (P5/P6/P7), P8 ships **no** per-pair tests against the P4
  reliability runner and **no** deferred P4-harness edges, so it carries the
  cross-phase reconciliation obligation (P4.77; reciprocal of P3.70/P5.74/P6.92/
  P7.77/P9.46) **inline on each box** rather than in a dedicated reconciliation box:
  P8.1.1→P2.39/P1.37 (`app://intake` + strings), P8.3/P8.16→P2.85 (`tauri-plugin-store`
  prefs blob), P8.10/P8.12→P2.34 (C11 `get_app_info`), P8.15→P2.33 (C10
  `open_project_page`), P8.19→P3.68 (§2.8.2 catalog), P8.20→P1.31.2/P3.69 (§5.1 store +
  §2.9.1 catalog). No P8 box `>`-note defers a `needs:` with the P4.77-forbidden
  phrasing (`the fill pass adds those needs` / `the reconciliation pass wires those`).

---

### App chrome (AppHeader / BrandLogo / ThemeToggle) — scope (i) ship-gating

- [ ] **P8.1** [UI] Build the persistent slim AppHeader chrome frame · §5.3 §5.5
  needs: P7.78
  > scope (i). The single persistent `AppHeader` present in every §5.2 state — slim, calm, never a second navigation model — laying out the three otherwise-homeless surfaces: BrandLogo (left), ThemeToggle + About/`?` trigger (right), with the BusyNotice Banner slot just under it (top of workspace). Renders identically across all 12 states.
  - [ ] **P8.1.1** [UI] Build the BusyNotice passive Banner (the §5.8 defence-in-depth leaked-`app://intake`-while-busy path) + its auto-dismiss rule · §5.3 §7.1.1 §5.8 · G57 G33a
    needs: P8.1, P2.39, P1.37
    > scope (i). The `BusyNotice.tsx` passive non-modal Banner in the AppHeader slot, fired **ONLY** on the §5.8 defence-in-depth path — a leaked `app://intake` arriving while not Idle/Summary (NOT the primary refuse-busy path, which the core handles core-side with no emit at the §7.1.1 PRIMARY `forward_launch_intake` funnel, P2.55 — P2.72 asserts that upstream delegation, it adds no separate core-side freeze gate); carries the §7.1.1 "ConvertIA is busy…" `strings/ui.ts` string (English-only, G57). Auto-dismiss rule: it persists across the Converting family (7→7a Cancelling does **NOT** dismiss it) and clears on **leaving** the Converting family (→ Summary(8) / AppFault(12)). Passive (never modal, never forces a choice — `role` is a status Banner, not `alertdialog`), the per-push `vitest-axe` ARIA target (G33a). (Build-order: `needs: P2.39` for the `app://intake` event binding + `P1.37` for the `strings/ui.ts` scaffold the §7.1.1 string lands in, per the P8 boundary note — the cross-phase edges declared, not deferred.)
- [ ] **P8.2** [UI] Build the BrandLogo primitive reading the bundled offline placeholder SVG · §5.5
  needs: P8.1
  > scope (i) (ship-gating: the header is a ship surface; the *final mark* is scope-(ii) branding). A single `<BrandLogo>` primitive reading a bundled-local placeholder SVG (offline, no CDN, §2.11) so the owner can swap the final Ne-IA mark without touching layout; the logo + "ConvertIA"/"Ne-IA" names are NOT MIT-granted (SSOT Trademark) — placeholder stand-in only.
- [ ] **P8.3** [UI] Build the ThemeToggle (Light/Dark/System) writing the `theme` prefs key · §5.5 §7.4.2
  needs: P8.1, P2.85
  > scope (i). The Light/Dark/System selector in AppHeader (right side); three explicit states; default `system` (follow `prefers-color-scheme`); writes the `theme` key via `tauri-plugin-store` (§7.4.2) so the choice persists across launches; cycles `system → light → dark`. Tab-reachable only, no global accelerator (§5.10) — wired to the keymap in P8.18. (`needs: P2.85` — the `tauri-plugin-store` 3-key prefs blob this `theme` key writes into.)
- [ ] **P8.4** [UI] Resolve the persisted `theme` into the design tokens at the root · §5.5 §7.4.2
  needs: P8.3
  > scope (i). Read the persisted `theme` at startup / store-hydration; a `system` value follows the OS `prefers-color-scheme`; resolve the chosen mode into the `design/tokens.css` colour tokens at the root so light + dark both render from one semantic token set (`theme.ts` light/dark resolution).

### Design system completion (tokens / motion / a11y floors)

- [ ] **P8.5** [UI] Finalize the semantic colour-token contract for light + dark · §5.5 §5.6 · G9
  > scope (i) (the *token contract* is the ship surface; the exact palette hex is scope-(ii) polish). Define the semantic colour tokens in `design/tokens.css` (`--bg`/`--surface`/`--surface-raised`/`--border`/`--text`/`--text-muted`/`--accent`/`--accent-contrast` + state colours `--success`/`--warn`/`--danger`/`--info`) for BOTH themes, surfaced to Tailwind via theme config so components use semantic classes, not raw hex (G9 invariant (a): no hardcoded colour outside `design/tokens.css`). Lossy/divert notes use `--info`/`--text-muted` (calm, never `--danger`); failures use `--danger`.
- [ ] **P8.6** [UI] Pin the typography scale with the `--text-base = 16px` body floor · §5.5 §5.6
  needs: P8.5
  > scope (i) (a11y floor is a ship gate). Single clean UI sans (system stack + bundled-offline fallback so it renders identically with zero network, §2.11); sizes `--text-xs … --text-2xl`, line-heights, weights `regular/medium/semibold`; the DECIDED body floor `--text-base = 1rem (16px)` is the minimum for body copy, `--text-xs`/`--text-sm` reserved for supplementary labels only — the AA "readable text sizes" half the §6.6 walkthrough verifies against.
- [ ] **P8.7** [UI] Author the spacing / radius / elevation token groups · §5.5
  needs: P8.5
  > scope (i). The 4-px-base spacing scale (`--space-1`=4 … `--space-8`=32, Tailwind default kept), the `--radius-sm/md/lg` rounding tokens (cards/tiles/DropZone — generous for the modern feel), and the elevation/shadow tokens — the structural token contract components consume.
- [ ] **P8.8** [UI] Wire the motion/eye-candy budget honouring `prefers-reduced-motion` · §5.5 §5.6
  needs: P8.7
  > scope (i) (the reduced-motion gate is a11y ship; the easing *taste* is scope-(ii)). Subtle ≤200 ms ease-out transitions on DropZone (lift/glow on `dragActive`), tile selection, drawer open/close, state transitions — and the motion-token contract; `prefers-reduced-motion` honoured (all non-essential animation disabled/reduced, progress still updates its value without easing). NO fake/indeterminate crawl on the convert step (SSOT Visible progress) — an indeterminate spinner permitted ONLY for the brief `Collecting` step.

### About + NOTICE + Impressum (release-gating) — scope (i)

- [ ] **P8.9** [UI] Build the AboutDialog shell as a focus-trapped `role="dialog"` aria-modal · §5.9 §5.6 · G33a G44
  needs: P8.1
  > scope (i), RELEASE-BLOCKING (SSOT §4 static About/legal-notices; G44 governance-completeness). The static in-app About / legal-notices dialog — presentation only, generates nothing. `role="dialog"` + `aria-modal="true"` (NOT `alertdialog` — it forces no decision; an alert role would be a WCAG 4.1.2 violation, §5.6), focus-trapped, `aria-labelledby` → its "About ConvertIA" heading, Esc-dismissible. Opened from the AppHeader About/`?` control + F1/`?` (§5.10) — keymap wiring in P8.18.
- [ ] **P8.10** [UI] Render the About version + build-id + MIT copyright lines from C11 `get_app_info` · §5.9 §7.6.2 · G33a
  needs: P8.9, P2.34
  > scope (i), RELEASE-BLOCKING. About checklist items 1–2: the current app version + build identifier (C11 `get_app_info`, §7.6 supplies the value, §5.9 renders) and the copyright line matching the repo `LICENSE`/`NOTICE` (the MIT copyright holder). The frontend treats C11 as an opaque typed RPC (§5.8) via `src/lib/ipc/**` only. (`needs: P2.34` — the C11 `get_app_info` contract this renders.)
- [ ] **P8.11** [UI] Author the canonical as-is/no-warranty + offline + cloud-sync About strings · §5.9 §5.7 §2.11 · G57
  needs: P8.9
  > scope (i), RELEASE-BLOCKING. About checklist items 3–5 as fixed `strings/ui.ts` entries (not free-form prose, so the §6.10 Principle-11 / drift lint covers them, G57): the canonical no-warranty string "ConvertIA is provided as-is, with no warranty. Use at your own risk."; the "fully offline" reassurance line (the `idle_reassurance`-class fixed string); the §2.11.3 cloud-sync caveat (your own OneDrive/iCloud/Dropbox may sync originals/results — ConvertIA neither causes nor prevents it) + the adjacent best-effort-security line.
- [ ] **P8.12** [UI] Build the scrollable third-party-licenses / NOTICE area presenting the §3.7 data · §5.9 · G44
  needs: P8.9, P2.34
  > scope (i), RELEASE-BLOCKING (a missing attribution is release-blocking, SSOT §9 / G44). About checklist item 6: a scrollable list rendering the §3.7-generated NOTICE/SBOM data (engine name → licence → notice text), with copyleft engines flagged per §3.6 + the written-offer-of-source pointer line. Data ships as a bundled offline asset (no fetch); About displays, never generates. C11 `get_app_info` supplies the NOTICE data. (`needs: P2.34` — the C11 `get_app_info` contract supplying the NOTICE data.)
- [ ] **P8.13** [UI] Render the Ne-IA branding block in About (logo placeholder + name) · §5.9 §5.5
  needs: P8.9, P8.2
  > scope (i) (the credits/trademark block is a ship surface; final art is scope-(ii)). About checklist item 8: the `<BrandLogo>` placeholder + "ConvertIA" name credits block — the home for credits + third-party-licenses (SSOT: no operated service → no web-style legal-notice obligation, so the in-app screen is the home). Logo + names NOT MIT-granted (SSOT Trademark).
- [ ] **P8.14** [UI] Author the Impressum content as bundled offline About content · §5.9 · G57
  needs: P8.12
  > scope (i), ship-gating. The Impressum (legal-notice/credits) content — fixed `strings/ui.ts`-owned text (English-only, §5.7, lint-covered by G57), rendered in the AboutDialog credits area alongside the §3.7 NOTICE list as a bundled offline asset (no fetch). It is part of the static About/legal-notices home (SSOT: the in-app About screen is where credits live; no operated-service legal-notice obligation).

### About→Releases link (§7.6.2) — scope (i)

- [ ] **P8.15** [UI] Build the user-initiated "Open Releases page" About link via C10 `open_project_page` · §5.9 §7.6.2 · G44
  needs: P8.9, P2.33
  > scope (i), ship-gating. About checklist item 7: a user-initiated link to the canonical Ne-IA GitHub Releases page (C10 `open_project_page` → the §7.7 `open_url` shell-out, the ONLY permitted, explicitly-user-triggered network action). NEVER an automatic check, no fetch/parse of the page itself (§7.6.1 no phone-home). The frontend calls C10 as an opaque typed RPC via `src/lib/ipc/**`; the no-`OpenKind` C10 row (§7.7.1) is the wiring point. (`needs: P2.33` — the C10 `open_project_page` contract this link invokes.)

### Settings chrome (verbose-log toggle) — scope (i)

- [ ] **P8.16** [UI] Build the verbose-logging toggle in About with the "applies after restart" hint · §5.9 §7.4.2 · G57
  needs: P8.9, P2.85
  > scope (i), ship-gating settings chrome. About checklist item 9: the "Detailed diagnostic log" labelled toggle (§7.5.3 mandate) with its disclosure notice (turning it on makes the LOCAL log additionally record file paths + engine command lines, still purely local — nothing sent, §2.11); off by default. Persists as the 3rd key (`verboseLog`) in the §7.4 prefs blob; takes effect on next launch (the setup stage resolves the verbose level once at startup) so it shows the "applies after restart" hint (§7.5.3). The toggle is the §7.5.3 SURFACE; logging behaviour is owned by §7.5. Labels are `strings/ui.ts` (G57). (`needs: P2.85` — the `tauri-plugin-store` 3-key prefs blob this `verboseLog` key persists into.)

### Cross-cutting error / edge-state copy refinement — scope (i)

- [ ] **P8.17** [UI] Refine the Idle empty-state, RENDERING the P1.37-owned `idle_reassurance` string · §5.2 §5.7 · G57
  needs: P1.37
  > scope (i) (the offline-promise string is ship; the empty-state eye-candy is scope-(ii), P8.27). The `Idle` (state 1) drop-or-browse invitation that **renders the P1.37-owned `idle_reassurance` `strings/ui.ts` key** (the SSOT Local/private/offline promise — authored + G57-lint-covered in P1.37, NOT re-defined here; P8 owns no string keys per the P8 boundary, it consumes the module). No setup, no fields. (`needs: P1.37` — the strings module that owns the key this state renders.)
- [ ] **P8.18** [UI] Fill the canonical keymap entries P8 chrome owns into `a11y/keymap.ts` · §5.10
  needs: P8.3, P8.9, P8.15
  > scope (i). Register into the single-source `a11y/keymap.ts` the §5.10 accelerators the P8 chrome introduces: F1 / `?` → AboutDialog (any state, when no text field focused); the ThemeToggle Tab-to-then-Enter/Space activation (no dedicated chord, §5.5); AboutDialog Esc-close-and-restore-focus-to-trigger (the About control in AppHeader). The keymap is the single source; per-component code only references it. With this box the keymap is FINALIZED (the §5.10 table is filled incrementally by P5–P10 and completed here), so the completeness gate below binds.
  - [ ] **P8.18.1** [GATE] Assert every §5.10-enumerated accelerator resolves to a non-empty `keymap.ts` entry (the keymap analogue of the strings/error-kind/lossy-kind/IPC completeness gates) · §5.10 · G22 G24
    > the missing single-source completeness gate for the §5.10 accelerator table — the peer of the IPC-surface gate (plan-lint check 12 "C1–C13 complete"), the three-event invariant (P2.41), the ErrorKind catalog (P3.68 "every variant has a row" `#[test]`), and the LossyKind catalog (P3.69 same): a `#[test]`/Lane-A structural check that **parses the §5.10 canonical accelerator enumeration** and FAILS if any §5.10-named accelerator has a missing or empty `a11y/keymap.ts` entry. Distinct from P9.17 (which proves every action is keyboard-OPERABLE — reachability) and P11.9 (the human keyboard pass): this asserts the CANONICAL MAP matches the spec list, so a spec-named accelerator silently omitted from `keymap.ts` is a deterministic gate failure, not a maybe-human-catch. Ships a **G24 positive+negative self-test** (deleting a §5.10 accelerator from `keymap.ts` MUST fail; the finalized clean map MUST pass) registered in `scripts/gate-selftests/` so plan-lint check 16 is satisfiable. Homed here because the keymap is finalized at P8.18; the parent is `[x]` only when this sub-box is (_format.md §2).
- [ ] **P8.19** [UI] Refine the cross-cutting §2.8 error / edge-state copy presentation (not per-format) · §5.2 §5.7 §2.8 · G57
  needs: P3.68
  > scope (i). Cross-cutting polish of the §2.8 error / edge-state SURFACING that is not a per-format declaration, rendered verbatim from the §2.8-owned catalog (the §2.8.2 table authored in P3.68; UI must not paraphrase), with UI-chrome wrapper strings in `strings/ui.ts` (English-only, G57). Refines presentation against the P4-built error-copy framework; does not re-author the catalog (§2.8 owns it — `needs: P3.68`). Split into three independently-verifiable sub-boxes by surface (different components, different spec refs: §5.2/§5.3 states 9/10/12 vs §1.4 vs §1.12) so a failure is attributable.
  - [ ] **P8.19.1** [UI] Refine the full-screen error-state copy — UnsupportedNotice (10) four variants + MixedDropRefusal (9) + AppFaultNotice (12) · §5.2 §5.3 §2.8 · G57
    > scope (i). The full-screen `UnsupportedNotice` (state 10) four variants (`Unsupported`/`Uncertain`/`Unreadable`/`Empty`) incl. the `Empty` per-reason skip-tally line, the `MixedDropRefusal` (state 9) formats-found list copy, and the `AppFaultNotice` (state 12) "Something went wrong" line — verbatim from the §2.8.2 catalog (P3.68), UI-chrome wrappers in `strings/ui.ts` (G57).
  - [ ] **P8.19.2** [UI] Refine the inline / partial-error copy — CommandError pre-run slot + the §1.4 confirm-gate skip tally · §5.3 §1.4 §2.8 · G57
    > scope (i). The `CommandError` pre-run inline-error slot copy (the passive `Note` above FormatPicker for a C3/C4/C5 reject) and the §1.4 confirm-gate skip tally — verbatim from the §2.8.2 catalog (P3.68), UI-chrome wrappers in `strings/ui.ts` (G57).
  - [ ] **P8.19.3** [UI] Refine the Summary / batch-level copy — fully-failed banner + the §1.12 batch-level summary strings · §1.12 §2.8 · G57
    > scope (i). The fully-failed `Summary` banner and the §1.12 batch-level summary strings (all/partial/all-failed/cancelled/with-residue) — verbatim from the §2.8.2 catalog (P3.68), UI-chrome wrappers in `strings/ui.ts` (G57).
- [ ] **P8.20** [UI] Refine the cross-cutting §2.9 lossy/fidelity-note presentation (not per-format) · §5.7 §2.9 §5.1 · G57
  needs: P1.31.2, P3.69
  > scope (i). Cross-cutting polish of the §2.9 lossy-note SURFACING that is not a per-format declaration: the passive inline `Note` calm/`--info` styling beside the chosen target (shown once, never a blocking "I understand" dialog or per-conversion nag), the multi-kind de-dup-to-most-specific-2-3 rule (§2.9.2), and the `ConvertingNote` worst-case-`video_reencode` banner reading the §5.1 store `pendingVideoReencodeNote` field (§5.8; the store is P1.31.2) — verbatim from the §2.9-owned string catalog (the §2.9.1 table authored in P3.69; UI must not paraphrase). Refines presentation against the P4-built lossy-note surfacing; does not re-author the catalog (§2.9 owns it — `needs: P3.69`).

### Scope-(i) ship-gating sub-gate

- [ ] **P8.21** [DOC] Record the "P8 ship-gating done" sub-gate (scope (i) complete) · §5.9 · G44
  needs: P8.10, P8.11, P8.12, P8.13, P8.14, P8.15, P8.16, P8.17, P8.18, P8.19, P8.20
  > scope (i) closure (README P8 fill-pass note: "a clear P8 ship-gating done sub-gate; so 'P8 done for release' is unambiguous"). Record in this plan that every release-blocking scope-(i) surface is built: About + NOTICE attribution (P8.10–P8.13) + Impressum (P8.14), About→Releases (P8.15), settings chrome (P8.16), cross-cutting error/lossy/empty-state refinement (P8.17/P8.19/P8.20) + the keymap (P8.18). This is the line that makes scope (i) (ship) separable from scope (ii) (non-blocking polish). G44 governance-completeness covers the About/NOTICE leg at release.

### Visual-polish / Ne-IA branding pass — scope (ii) NON-BLOCKING (may trail P11)

- [ ] **P8.22** [UI] Apply the finalized colour palette + accent over the semantic tokens · §5.5
  needs: P8.5
  > scope (ii) — NON-BLOCKING, may trail the P11 RC (SSOT §9: visual polish is "Not a gate"). The owner-finalized palette hex values applied to the semantic tokens from P8.5 (the token *contract* is fixed in P8.5; only the placeholder *values* change here), both themes. Purely the modern-clean palette pass; no structural change.
- [ ] **P8.23** [UI] Swap in the final Ne-IA brand mark for the BrandLogo placeholder · §5.5 §5.9
  needs: P8.2, P8.13
  > scope (ii) — NON-BLOCKING, may trail the P11 RC (SSOT Design Intent: logo/colours/branding are placeholders for now). Replace the placeholder SVG behind `<BrandLogo>` with the final Ne-IA mark (still a bundled-local offline asset, no CDN, §2.11); layout untouched (the placeholder was built so the swap touches no layout). Names/logo remain NOT MIT-granted (SSOT Trademark).
- [ ] **P8.24** [UI] Polish the DropZone modern styling + dragActive lift/glow · §5.3 §5.5
  needs: P8.7, P8.8
  > scope (ii) — NON-BLOCKING, may trail the P11 RC. The "modern > plain" visual pass on the DropZone (generous rounding, the subtle lift/glow on `dragActive` within the ≤200 ms motion budget, calm contemporary look). The DropZone's structure + a11y + keyboard activation were built in P4; this is the eye-candy layer only, honouring `prefers-reduced-motion` (P8.8).
- [ ] **P8.25** [UI] Polish the FormatPicker target-tile selection styling · §5.3 §5.5
  needs: P8.7, P8.8
  > scope (ii) — NON-BLOCKING, may trail the P11 RC. The modern-clean visual pass on the FormatPicker target tiles (tile selection transition, the pre-highlighted-default emphasis, disabled-tile dimming-with-reason styling). The radiogroup/roving-tabindex/ARIA + descriptor-driven rendering were built in P4/P5–P7; this is visual taste only — disabled tiles stay visible-with-reason (never just dimmed, §5.6).
- [ ] **P8.26** [UI] Polish the progress / convert-screen chrome styling · §5.3 §5.5
  needs: P8.7, P8.8
  > scope (ii) — NON-BLOCKING, may trail the P11 RC. The visual pass on the `Converting` screen — the determinate per-item + aggregate ProgressBar smooth value animation (never a fake crawl), current-item label, the calm `ConvertingNote` banner styling, the live Cancel button. The progress/cancel mechanics + a11y (`role="progressbar"` + `aria-valuemin/max/now`) were built in P4; this is styling only, honouring `prefers-reduced-motion`.
- [ ] **P8.27** [UI] Add the restrained empty-state eye-candy to Idle + Summary · §5.2 §5.5
  needs: P8.8, P8.17
  > scope (ii) — NON-BLOCKING, may trail the P11 RC (SSOT marks "modern/eye-candy" polish non-blocking). The restrained empty-state visual flourish on `Idle` (1) (around the drop invitation + reassurance line) and the `Summary` (8) outcome screen — uncluttered, a little eye candy, never busy; honours `prefers-reduced-motion` (P8.8). The functional copy/states are owned by scope-(i) boxes; this is the decorative layer only.

---

### The phase-end Co-Pilot hardening sweep — the standing phase-close box

> The standing test-strategy §11 phase-close box (owner directive, recorded 2026-07-06):
> Co-Pilot-executed — never the Build-Loop; mandate, level and evidence rules in
> [test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep).

- [!extern] **P8.28** [TEST] Run the phase-end Co-Pilot hardening sweep over the whole P8 delivery — adversarial re-test at the hardest technically-possible level · §6.4
  > **[!extern] (Co-Pilot-executed — the standing test-strategy §11 phase-close sweep, never the Build-Loop):** runs once every other P8 box is `[x]`; the phase's whole delivery is adversarially re-tested at the hardest technically-possible level with unrestricted session tooling (Docker, WebDriver/Playwright, property/fuzz/mutation probes, real-OS live runs); findings are fixed with tests as normal dual-reviewed commits before this box flips `[x]`.
  > **Boundary stop:** P9.1 carries `needs:` on this box — a `[!extern]` prerequisite of a non-extern box is a loop STOP (`_format.md` §2/§6), so the loop hard-stops at the P8→P9 boundary and hands off to the Co-Pilot until the sweep is `[x]`.
