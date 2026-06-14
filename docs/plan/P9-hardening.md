# P9 — Hardening (performance · validation · security · corpus)

> **Goal:** ConvertIA meets its **non-functional contracts** and the **deferred
> empirical items** are validated. P9 *exercises* the controls earlier phases built —
> the §2.12 isolation boundary (P4), the per-engine §3.5.x SSRF/LFR argv/build
> controls (P5–P7), the strings module + structural-a11y wiring (P1/P4), the
> guarantees-fs byte-verbatim-stem / path-limit mechanism (P2/P3) — and **introduces
> no new isolation mechanism**. It stands up the **§6.4.6 headed-E2E infrastructure**
> (the scaffolding that produces the validation outputs), runs the **a11y / fidelity /
> egress / fuzz** validations, verifies the **§0.11 threat map**, and calibrates the
> **`[DEFER: corpus]`** resource/budget numbers against the §6 corpus.
>
> **Spec home:** [`02-guarantees.md`](../spec/02-guarantees.md) (§2.10 filename/
> content fidelity, §2.11.4 offline-egress observability, §2.12.3 privilege-drop
> tier), [`05-ui-ux.md`](../spec/05-ui-ux.md) (§5.6/§5.6.1 a11y validation),
> [`06-build-test-release.md`](../spec/06-build-test-release.md) (§6.4 corpus, §6.4.2
> property/fault-injection + adversarial-egress, §6.4.6 + §6.4.6a headed-E2E infra +
> automated a11y, §6.6 human walkthrough evidence, §6.7.3 egress gate), and
> [`01-conversion-pipeline.md`](../spec/01-conversion-pipeline.md) §1.10 +
> [`03-engines-and-bundling.md`](../spec/03-engines-and-bundling.md) §3.9 (resource /
> size budgets). Threat-map: [`00-architecture.md`](../spec/00-architecture.md) §0.11.
> Box format: [`_format.md`](_format.md). Index: [README.md](README.md).
>
> **This is the v0 base** — the smallest atomic `[ ]` boxes below, grouped under
> `### ` sub-headings, worked top to bottom; a later adversarial-review pass deepens,
> splits and reconciles them (incl. P0's `→ activated in P9` / `→ executed in P9`
> cross-refs from P0.5.9 / P0.7.11 / P0.7.12 / P0.7.14 / P0.7.15, which name P9 as the
> activation target for the egress-deny window, the privilege-drop-tier ratchet, the
> release-confirmation G42/G42b leg, the G33b contrast scan, and the engine-fuzz job).

## Boundaries (read against P1/P4/P5–P7/P8)

- **P9 ↔ P1/P4:** P1 built `strings/ui.ts` + the TS gate contracts; P4 wired the
  structural-a11y chrome (ARIA roles, roving-tabindex radiogroup, the focus-on-entry
  rules, the `a11y/announcer.ts` live region) + the §2.12 isolation wrapper + the
  per-push jsdom `vitest-axe` leg (G33a). **P9 does not re-author** any of these — it
  builds the **live-WebView** validation harness and runs the validations the
  per-push leg cannot (computed contrast, the full driven flow, the keyboard-path
  equivalence).
- **P9 ↔ P5–P7:** P5–P7 staged each engine + authored its §3.5.x SSRF/LFR argv/build
  controls + per-push adversarial-egress pull-forward leg (P0.7.12 leg (b)). **P9
  builds the full per-OS egress-DENY window + the armed-window canary + the
  release-confirmation G42/G42b leg** (P0.7.12 leg (c)) that runs every engine's
  adversarial corpus inside it — it does not re-decide any argv control.
- **P9 ↔ P8:** P8 owns the full UI experience + visual polish. **P9 validates** the
  a11y/keyboard floor against the built UI; it does not add UI surfaces.
- **Reads, never re-decides:** the §1.10 resource-budget **design** (ceilings,
  1.3× headroom, ~10 s GIF cap — only the *numbers* are corpus-calibrated here), the
  §3.9.2 ≤400 MB artifact budget (the *gate* runs at release in P10; P9 measures the
  trend), and the §2.12.3 privilege-drop **tier model** (`[DECIDED]`; only the
  achieved-tier matrix is filled here).

---

### Headed-E2E infrastructure (§6.4.6 — the scaffolding that produces the validations)

- [ ] **P9.1** [TEST] Pin WebdriverIO v9 + `@axe-core/webdriverio` + the E2E client deps in the pnpm workspace · §6.4.6 §6.4.6a · G18c G18d
  > add **WebdriverIO v9** (the W3C-WebDriver-only major, aligned with `tauri-driver`'s W3C session model) and **`@axe-core/webdriverio`** to the §0.8 dependency set, pinned in `pnpm-lock.yaml`; the contrast session (P9.16) reuses the same driver session as the flow E2E, so the JS client is mandatory (the Rust `webdriver`/`fantoccini` crate is rejected — `@axe-core/webdriverio` is JS-only, §6.4.6). Honours the P0.3.8 registry-pin + lifecycle-script lockdown (G18c/G18d).
- [ ] **P9.2** [TEST] Install + pin the `tauri-driver` binary (Windows + Linux only) via the gate-tool fetch-and-verify mechanism · §6.4.6 · G24
  needs: P9.1
  > `tauri-driver` exposes a **WebDriver** endpoint over the platform WebView (WebKitGTK Linux, WebView2 Windows); install it through the P0.2.1 pinned fetch-and-verify mechanism (exact version + checksum, its own G24 wrong-checksum self-test) and record the **`tauri-driver` minor against which the default port `4444` holds**. There is **NO macOS WKWebView driver** — macOS uses the degraded smoke leg (P9.7), so `tauri-driver` is Win/Linux only.
- [ ] **P9.3** [TEST] Author `wdio.conf.js` — `tauri:options` capabilities + the `tauri-driver` host/port binding · §6.4.6
  needs: P9.2
  > the WebdriverIO config: session configured with **`tauri:options`** (`application` = the built-app binary path + any `args`) and a `tauri-driver` host/port WebdriverIO connects to (it proxies to `msedgedriver` on Windows, `WebKitWebDriver` on Linux); **no Chrome/Firefox capability block**. The `${DRIVER_PORT:-4444}` default matches the P9.27.3 readiness-probe port so the two can never disagree.
- [ ] **P9.4** [TEST] Wire the Linux `tauri-driver` leg — AppImage extract + dynamic `productName` binary resolution + `squashfs-root` cleanup · §6.4.6
  needs: P9.3
  - [ ] **P9.4.1** [TEST] Extract the AppImage and point `application` at the extracted ELF, not the `.AppImage` · §6.4.6
    > the AppImage is a self-mounting wrapper WebDriver cannot launch as a process target, so CI runs `./ConvertIA.AppImage --appimage-extract` (or `--appimage-extract-and-run`) and points `tauri:options.application` at the extracted binary under `squashfs-root/usr/bin/`.
  - [ ] **P9.4.2** [TEST] Resolve the extracted binary name DYNAMICALLY from `productName` (glob `squashfs-root/usr/bin/*`) · §6.4.6
    > the extracted name matches the **case-sensitive Tauri `productName`** (e.g. `ConvertIA`, not `convertia`), so CI globs `squashfs-root/usr/bin/*` or reads `productName` from `tauri.conf.json` — a hardcoded lowercase name would not exist.
  - [ ] **P9.4.3** [TEST] Run `rm -rf squashfs-root/` after the E2E so the extracted tree never accumulates · §6.4.6
    > mandatory cleanup so the extracted tree does not contaminate the artifact/disk budget or carry across runs.
- [ ] **P9.5** [TEST] Wire the Linux E2E under `Xvfb` (WebKitGTK needs a display) · §6.4.6
  needs: P9.4
  > the headless Linux runner has no X/Wayland display, so the leg runs under **`xvfb-run -a ...`** (or a Wayland headless compositor); without it WebKitGTK never initialises and the E2E silently can't start. (The egress-window composition with `Xvfb` is P9.13 — this box wires the display for the *non*-egress Lane-A/Lane-B flow leg.)
- [ ] **P9.6** [TEST] Wire the Windows `tauri-driver` leg — `msedgedriver` matched to the runner's WebView2/Edge runtime · §6.4.6
  needs: P9.3
  > a mismatched `msedgedriver` fails to attach, so the CI step **resolves the runner's installed WebView2/Edge build and fetches the matching `msedgedriver`** (not a hardcoded version); pin + capabilities block finalised against the pinned `tauri-driver` minor.
- [ ] **P9.7** [TEST] Wire the macOS degraded smoke leg — synthetic-argv conversion + window/output/exit-0 assertions · §6.4.6 §6.4.4
  needs: P9.1
  > `tauri-driver` has no macOS WKWebView driver, so the macOS automated leg **launches the built app, drives a synthetic `argv` conversion of one corpus file through the launch-intake path (§7.8/§1.1), and asserts (a) window/process present, (b) expected output file appears, (c) exit 0** — a scripted launch + presence/output assertion, NOT a WebDriver flow. (`safaridriver` automates Safari itself, not an embedded WKWebView — it does not apply.) The macOS WebView UX flow is covered by the §6.6 human walkthrough (evidence recorded by P9.45).
- [ ] **P9.8** [TEST] Handle the macOS smoke quarantine + TCC constraints (build-output dir, no zip round-trip; temp-dir-only writes) · §6.4.6 §6.4.4
  needs: P9.7
  > run the smoke test on the **build-output `ConvertIA.app` directly** (no archive/re-extract → no `com.apple.quarantine`, no `spctl`/`xattr` bypass needed); IF the pipeline zips/re-unzips first, run `xattr -rd com.apple.quarantine ConvertIA.app` before launch. **TCC:** prompts cannot be answered headlessly, so the smoke leg writes/reads a **`TMPDIR`/temp dir only** (no Desktop/Documents/Downloads) where no TCC prompt fires; the TCC-prompt exercise moves to the §6.6 human walkthrough (evidence recorded by P9.45).
- [ ] **P9.9** [TEST] Author the driven §5.2 full-flow E2E via the file-picker path (native drop is NOT automatable) · §6.4.6 §5.2
  needs: P9.5, P9.6
  > the Win/Linux WebDriver run exercises the full §5.2 flow per platform: empty → intake → collected/confirm → target+default → destination shown → progress → summary → open-folder. **Native file-drop cannot be synthesised by WebDriver**, so the automated E2E uses the **file-picker path** (C2a `pick_for_intake` via the §5.10 accelerator → the same C1 `ingest_paths` as a drop, §1.1); the native drop itself is validated in the §6.6 human walkthrough (evidence recorded by P9.45).
- [ ] **P9.10** [TEST] Assert the Idle "all conversion happens locally, on your machine" reassurance line is present in the E2E · §6.4.6 §5.2 §5.7
  needs: P9.9
  > a cheap string-presence check on the empty/Idle step (SSOT *Offline/privacy* surfaced on Idle, §5.2 row 1 / §5.7) so the offline reassurance can't silently drop — the automated half of the DoD core-UX-flow gate.

---

### A11y validation (§6.4.6a / §5.6 / §5.6.1 — live-WebView + keyboard + SR floors)

- [ ] **P9.11** [TEST] Stand up the `@axe-core/webdriverio` contrast session reusing the §6.4.6 driver session (Linux + Windows) · §6.4.6a §5.6 · G33b
  needs: P9.9
  > the contrast scan runs on the **live WebView** (jsdom cannot compute contrast, so this is NOT the per-push G33a leg) reusing the same `tauri-driver` session as the flow E2E; the macOS contrast gap is acknowledged (no WKWebView driver) and satisfied only by the §6.6 human walkthrough, recorded in `docs/usability-floor.md`.
- [ ] **P9.12** [TEST] Run the WCAG 2.1 AA contrast assertion in BOTH Light and Dark themes · §6.4.6a §5.5 §5.6 · G33b
  needs: P9.11
  > axe `color-contrast` — **≥4.5:1 normal text, ≥3:1 large text + UI components/graphical objects** — run in both themes (§5.5); the rendered colours come from the §5.5 design tokens, so this is what makes the §5.6 "WCAG 2.1 AA" claim verifiable. Any violation at the configured impact level fails the build.
- [ ] **P9.13** [TEST] (Optional belt-and-suspenders) computed-`font-size` ≥16px assertion on the live `@axe-core/webdriverio` session · §6.4.6a §5.5 §5.6 · G33b
  needs: P9.11
  > axe does not check font size; the operative text-size gate is the §6.6 human walkthrough (evidence recorded by P9.45), but an optional Lane-B computed-`font-size` assertion (no main-content text element below 16px = `--text-base`, §5.5) may be added on the live session as defence-in-depth — body copy must use `--text-base` or larger, `--text-xs`/`--text-sm` reserved for supplementary labels.
- [ ] **P9.14** [TEST] Validate the §5.6.1(1) mandatory-ARIA-role contract against the live driven tree · §5.6.1 §5.6 · G33b
  needs: P9.9
  > walk the §5.6.1(1) role table on the driven WebView: DropZone `role="button"`; FormatPicker `role="radiogroup"`+`aria-labelledby`; target tile `role="radio"`+`aria-checked` (disabled tile `aria-disabled="true"`, kept in arrow order); ProgressList row `role="progressbar"`+`aria-valuemin/max/now` (LibreOffice indeterminate row `aria-busy="true"`, cleared on terminal); the three focus-trapped dialogs (RerunPrompt/QuitConfirm `role="alertdialog"`, AboutDialog `role="dialog"`+`aria-modal`) each named via `aria-labelledby`→heading; MixedDropRefusal/UnsupportedNotice/AppFault full-screen states with `aria-live="assertive"` headings (no `alertdialog`). A missing/wrong role is a §6.4.6a failure.
- [ ] **P9.15** [TEST] Validate the §5.6.1(2) assertive-announcement-on-entry set on the live driven flow · §5.6.1 §5.6 · G33b
  needs: P9.14, P4.66.1, P4.74
  > assert the states that MUST announce assertively on entry do (Confirm 3, RerunPrompt 6, Summary 8 + first `Failed` row, MixedDropRefusal 9, UnsupportedNotice 10, QuitConfirm 11 — the component BUILT in P4.66.1, AppFault 12) and the polite ones stay polite (Collecting progress, lossy/divert notes, throttled Converting milestones — no per-tick flood); `aria-busy` cleared to `false` + `aria-valuenow` set to 100/last-known on each item's terminal transition (WCAG 4.1.2). (`needs: P4.74` — the `announcer.ts` body + §5.6.1(2) announce-on-state-entry mechanism this box validates, split from P4.69.)
- [ ] **P9.16** [TEST] Validate the §5.6.1(3) per-state SR traversal table — every state has a non-orphaned landing element · §5.6.1 §5.6 §5.10 · G33b
  needs: P9.14
  > drive each of the 12 states and assert focus lands on the named primary element per the §5.6.1(3) table (Idle→DropZone, Collecting→cancel-or-`role="status"`, Confirm→Confirm button, Targets→default-checked radio then Convert-when-shown, RerunPrompt→Skip, Converting→Cancel, Summary→first-Failed/OpenActions/banner, the full-screen states→their landing per §5.6, QuitConfirm→Stay, AppFault→Start Over) — the "a screen-reader path exists" contract is non-orphaned at every state.
- [ ] **P9.17** [TEST] Validate the keyboard-path equivalence — every action operable keyboard-only via the §5.10 map · §5.6 §5.10 · G33b
  needs: P9.9
  > drive the full §5.2 flow keyboard-only through the §5.10 accelerator map (open picker, confirm batch, pick target via radiogroup arrow-roving, open Advanced, change destination, convert, cancel, open folder/file, dismiss a refusal, answer the re-run prompt) — no mouse-only affordance; modal focus-trap + Esc; the state-6 reducer-level suppression of global accelerators (Ctrl/⌘+N/O/Backspace) while RerunPrompt is open.
- [ ] **P9.18** [TEST] Validate disabled (patent-gapped) tiles stay in arrow-roving with `aria-disabled`+reason, never visual-dim-only · §5.6 §5.6.1 · G33b
  needs: P9.14
  > a patent-gapped tile is `role="radio"` `aria-disabled="true"` `tabindex="-1"` — kept in **arrow-key** roving (so a keyboard user can hear the *why* via `aria-describedby`) but out of the **Tab** sequence; never just visual dimming, never removed from arrow navigation (the roving-tabindex single-tab-stop contract intact).

---

### §2.10 real-world filename / content fidelity validation

- [ ] **P9.19** [TEST] Author the adversarial-filename §6.4.1 unit corpus — emoji / CJK / RTL / spaces / multi-dot / extension-only stems · §2.10.1 §6.4.1 · G15
  > unit tests over `fs_guard::output_name` proving the stem is preserved **byte-for-byte** (no transliteration, ASCII-folding, emoji-stripping): multi-dot (`my.report.final`→`.pdf`), extension-only-looking tokens, same-format re-encode (`photo.jpg`→`photo (1).jpg` never overwriting), and the space-paren `(n)` numbering shape — not `_1`/`-1`/a hash.
- [ ] **P9.20** [TEST] Validate path-as-opaque-OsString — no lossy `to_string_lossy()` in any FS *operation* (display-only at the last step) · §2.10.1 · G9 G15
  > assert ConvertIA operates on the original `OsString`/`PathBuf` and only converts to `String` for *display* to the WebView (replacement char shown, original operated on losslessly); covers Windows WTF-8/UTF-16 (emoji/CJK/combining-mark round-trip) and Unix arbitrary-byte paths. Pairs with the G9 invariant grep (P0.3.10) that no operation path drops to lossy.
- [ ] **P9.21** [TEST] Validate the macOS NFC-vs-NFD identity invariant — no missed-identity / duplicate from normalization · §2.10.1 §2.3.1 · G15
  > assert the stem is preserved verbatim (no cross-OS re-normalization) and the §2.3 identity check uses inode/file-index, NOT the name string, so an NFC-vs-NFD difference never causes a missed-identity or a duplicate frozen-set entry.
- [ ] **P9.22** [TEST] Validate `PathTooLong` fail-clearly — Windows 260 / 255-component, macOS 255-byte/PATH_MAX, Linux 255/4096 (no truncation) · §2.10.1 §2.2.3 · G15 G48
  > assert appending `(n)` / swapping the extension that would exceed the **component** or **total** limit emits `PathTooLong` (§2.8) — truncation is never the escape hatch — including on the §2.7 divert path (identical guarantee); the Windows `\\?\` extended-length prefix is used for ConvertIA's own syscalls but a user-facing path > 260 still fails clearly. Sits alongside the §6.4.5 bound-firing fixtures and the P0.4.3 `fs_guard` fuzz.
- [ ] **P9.23** [TEST] Validate CJK/RTL body-text fidelity through every document/sheet/slide pair against the bundled font floor · §2.10.2 §6.4.3 · G31 G32
  needs: P9.31
  > assert CJK + RTL (Arabic/Hebrew) body text survives the doc/sheet/slide conversions (§2.10) rendering from the **committed bundled font set alone** (§3.9.3: Liberation + Carlito + Caladea + curated Noto CJK/RTL) — a missing-font regression fails the gate rather than silently degrading to host-font substitution (no tofu); uses the `cjk-body`/`rtl-body` content-floor corpus tags (P0.4.11 / §6.4.5).
- [ ] **P9.24** [TEST] Validate text-encoding detection (BOM→declared→heuristic) + UTF-8 default; mixed/invalid bytes fail-clearly (no mojibake) · §2.10.2 §6.4.3 · G31 G32
  needs: P9.31
  > assert encoding is detected, never assumed from the extension (BOM → declared charset → heuristic UTF-8→Windows-1252/Latin-1→broader); output text defaults to UTF-8 (no BOM unless the target demands); a **mixed/invalid byte sequence** produces `Corrupt`/`EngineError` (§2.8) rather than mojibake — "mangled" output is never acceptable. Uses the `non-ascii-encoding` content-floor corpus (TXT non-UTF-8 + CSV/TSV non-ASCII).
- [ ] **P9.25** [TEST] Validate CSV encoding + delimiter (`,`/`;`/`\t`/pipe) detected-and-preserved, never silently re-delimited/re-encoded · §2.10.2 §6.4.3 · G31 G32
  needs: P9.31
  > assert per spreadsheets.md: semicolon (European decimal-comma), tab, pipe, UTF-8-BOM/UTF-16/Windows-1252 samples come through with delimiter + encoding intact; the leading `=`/`+`/`@` injection cells preserved literally as text (CSV-injection non-execution on the output side, via a real RFC-4180 reader — NOT bare field-count parity).
- [ ] **P9.26** [TEST] Validate audio/video tag fidelity (any script) + the honest `audio_tags_dropped` disclosure where a target can't store tags · §2.10.2 §6.4.3 · G31 G32
  needs: P9.31
  > assert non-Latin/CJK/RTL tag text round-trips through tag models that support UTF-8 (audio.md tag policy); where a target can't store tags, the §2.9 `audio_tags_dropped` note fires (a disclosed loss, not silent mangling). Uses the `non-latin-tags` content-floor corpus.

---

### Offline-egress observability gate (§2.11.4 / §6.7.3 — the per-OS deny window + observe-the-attempt)

- [ ] **P9.27** [TEST] Build the per-OS egress-deny window harness — observe-the-attempt, fail the release on any outbound attempt · §2.11.4 §6.7.3 · G42
  needs: P9.9
  - [ ] **P9.27.1** [TEST] Linux: net-namespace egress block with attempt-visibility (`unshare --net` loopback-only + `strace`/eBPF on connect, or `iptables … -j LOG`/`NFLOG`) · §2.11.4 §6.7.3 · G42
    > a bare `-j DROP`/`unshare --net` silently swallows the very packet the monitor needs — so pair the block with `strace`/eBPF on `connect()`/`socket()`/`sendto()` (or `iptables -A OUTPUT … -j LOG`/`NFLOG`, or an `ACCEPT`-to-black-hole sink + `tcpdump`) so a blocked-but-**attempted** egress is observable; any observed *attempt* fails the release.
  - [ ] **P9.27.2** [TEST] Linux: preflight `unshare --net true` assertion + clear diagnostic, fail-loud not silent-skip · §2.11.4 §6.7.3 · G42
    > `unshare --net` needs unprivileged userns (`kernel.unprivileged_userns_clone=1` / `user.max_user_namespaces>0`); a preflight assertion fails loud ("net-namespace unavailable — enable unprivileged userns or run with `--cap-add NET_ADMIN`") rather than silently skipping the isolation. If the VPS runner is containerised, use `--network=none` or `--cap-add NET_ADMIN` rather than in-container `unshare` (the §6.1.4 kernel-recording requirement is the cross-ref).
  - [ ] **P9.27.3** [TEST] Linux: compose the net-ns OUTSIDE `Xvfb`+E2E, bring `lo` up inside, readiness-probe + explicit kill (the pinned §6.7.3 form) · §2.11.4 §6.7.3 · G42
    needs: P9.5
    > the net-ns wraps the entire `Xvfb`+E2E process; `ip link set lo up` inside it; `xvfb-run -a --server-args="-nolisten tcp"` (Unix-domain X socket survives net-ns isolation, no TCP X socket inside loopback-only ns); a `curl -sf .../status` readiness probe before the WebdriverIO client + an explicit `kill` of the backgrounded driver + propagated exit code; `${DRIVER_PORT:-4444}` shared between launch and probe. Getting the nesting backwards yields no display or a half-isolated silently-passing gate.
  - [ ] **P9.27.4** [TEST] macOS: `pf` outbound-deny profile (`pass log`/`block log`→`pflog0` read by `tcpdump -i pflog0`) + passwordless-sudo runner-image assumption · §2.11.4 §6.7.3 · G42
    > a `pf` profile that **logs** matched outbound so the attempt is captured even while blocked; `pfctl` needs `sudo` (GitHub-hosted macOS runners have passwordless sudo, recorded as a §6.1.4 runner-image assumption — degrades to the packet-monitor alone if a future image drops it). macOS runs the synthetic-argv smoke (P9.7), NOT the WebDriver flow; the WebView CSP offline property on macOS is human-walkthrough + static-config only (the acknowledged §6.10 row 5 gap).
  - [ ] **P9.27.5** [TEST] Windows: per-run `New-NetFirewallRule -Program <resolved abs path> … -Action Block` (created+removed) or AppContainer no-network profile + packet-monitor as the load-bearing gate · §2.11.4 §6.7.3 · G42
    > a blanket firewall rule is fragile for a portable unsigned exe at a random `TEMP` path, so scope a per-run block rule to the run's actual exe path (created and removed around the test) **or** launch inside an AppContainer with no network capability; a Job Object is NOT a network-deny mechanism (`JOB_OBJECT_LIMIT` governs memory/CPU/process/UI, not sockets). The firewall/AppContainer is best-effort enforcement — the **§2.11.4 packet-monitor assertion is the real load-bearing gate** on Windows. Run both.
- [ ] **P9.28** [TEST] Add the REQUIRED DNS-only sub-assertion to the deny window (zero DNS over the engine PID scope) · §2.11.4 §6.7.3 · G42
  needs: P9.27
  > `tcpdump -i any port 53` (Linux/macOS) / Windows ETW `Microsoft-Windows-DNS-Client` over the engine PID scope: **zero DNS in the deny window**, with an armed-window canary + a resolver-cache flush so the absence is proven, not coincidental.
- [ ] **P9.29** [TEST] Run the benign full-conversion offline assertion — every category produces zero outbound packets (T9a) · §2.11.4 §6.7.3 · G42
  needs: P9.27
  > the §6.4.6 driven E2E (Win/Linux) + the §6.4.6 synthetic-argv smoke (macOS) run **inside the deny window** over the benign corpus: a full conversion of every category produces zero outbound packets and the app launches+converts identically with networking disabled — proves **T9a** (ConvertIA's own code opens no socket) and catches an accidental fetch. The packet-monitor "zero attempts observed" is the load-bearing proof (not "zero packets left the box", which the deny would hide).
- [ ] **P9.30** [TEST] Run the §7.2.3 startup `--version` smoke-probe + warm-launch within the deny window under the §3.5 minimal env · §2.11.4 §7.2.3 §3.5 · G42 G46
  needs: P9.27
  > the startup probes **spawn third-party engine binaries**, so to prove "zero startup network" they run inside the same packet-monitor / egress-deny window, each spawned with the §3.5 minimal env (no `http_proxy`/`https_proxy`/`*_PROXY`, `LD_PRELOAD`/`DYLD_*` stripped) — "zero startup network" is observably enforced for engine *spawns*, not only full conversions. (G46's startup-integrity verifier is wired in P4; this is its in-egress-window smoke.)

---

### Adversarial-egress + decoder-isolation / fuzz validation (§6.4.2 — exercising the P4/P5–P7 controls)

- [ ] **P9.31** [TEST] Assemble the §6.4.2 adversarial-network corpus (HLS/DASH/concat/ext-ref MP4, remote-include doc, remote/OLE + WEBSERVICE() office, remote/local-`../`-escape SVG) · §6.4.2 §0.11 · G24a
  > a small adversarial-network corpus per the §3.5.1/§3.5.4/§3.5.2/§3.5.5 surfaces: FFmpeg HLS `.m3u8` / DASH `.mpd` / `-f concat` script / external-reference-box MP4; pandoc remote-`<img>`/RST-include; LibreOffice remote/OLE-link office file **AND** a `WEBSERVICE()`/external-data-range `.xlsx`; librsvg remote-`href` **AND** an external local-`<image href>` `../`-escape; plus a known **out-of-input sentinel file** the engine must NOT read/embed. Manifest-tracked + SHA-256-integrity-verified (G24a, P0.5.4).
- [ ] **P9.32** [TEST] Run the adversarial corpus inside the egress-deny window — assert (a) zero outbound packets AND (b) no out-of-input file read (T9b) · §6.4.2 §2.11.4 §0.11 · G42 G42b
  needs: P9.27, P9.31
  > the network-trigger inputs are converted inside the §6.7.3 packet-monitor / egress-deny window and must produce **(a) zero outbound packets** (egress half) AND **(b) no out-of-input file read** (the out-of-input-read half, asserted by the sentinel the engine must not read/embed) — proving the argv/build controls, NOT "all engines bundled" and NOT the degradable §2.12.3 tier, close T9b. This is the per-OS release-confirmation pull of the per-push leg (P0.7.12 leg (c)).
- [ ] **P9.33** [TEST] Build the G42b read-half fs-audit substrate with availability-probe + fail-closed + `::error::` diagnostic · §6.4.2 §2.12.3 §6.1.4 · G42b
  needs: P9.32
  - [ ] **P9.33.1** [TEST] Prefer `ptrace` via `docker --cap-add SYS_PTRACE` (or native on the §6.1.4 VPS) · §6.4.2 §6.1.4 · G42b
    > the no-out-of-input-read half typically uses `ptrace`/strace, **commonly blocked in CI containers** (no `SYS_PTRACE`) → it would silently not-enforce; run with `docker --cap-add SYS_PTRACE` or outside Docker on the self-hosted VPS runner.
  - [ ] **P9.33.2** [TEST] Landlock `{input ro, scratch rw}` fallback WITH a kernel≥5.13 / ABI≥1 availability probe (grant-is-enforcement) · §6.4.2 §2.12.3 · G42b
    > if `ptrace` is unavailable, restrict the decoder to `{input ro, scratch rw}` and treat the grant itself as the enforcement (an out-of-input open denied by the kernel = the engine's `EACCES`); but Landlock is a best-effort silent-degrade tier, so **probe first** that the kernel has Landlock (ABI ≥ 1, kernel ≥ 5.13) and that the ruleset applied — never assume the grant took.
  - [ ] **P9.33.3** [TEST] FAIL CLOSED if neither `ptrace` nor Landlock is available, emitting a GitHub Actions `::error::` annotation · §6.4.2 §6.1.4 · G42b
    > when the runner has no `SYS_PTRACE` AND no working Landlock, the fs-audit half has no enforcement mechanism → it MUST hard-fail the CI gate (the runner is misconfigured), never silently pass; before the non-zero exit, emit `::error::fs-audit cannot enforce: neither ptrace (SYS_PTRACE) nor Landlock (kernel ≥ 5.13) available on this runner — see §6.4.2`; §6.1.4 must record the Lane-B VPS runner's kernel version + which enforcement path it uses.
- [ ] **P9.34** [TEST] Run the malformed/adversarial input fault-injection through the §2.12 boundary — one plain message, no crash, no wedge, batch continues · §6.4.2 §2.12 §2.13 · G26 G31
  needs: P9.31
  > truncated / 0-byte / fuzzed-header / encrypted-DRM (password PDF/XLSX/PPTX, FairPlay M4V, PlaysForSure WMV) / decompression-bomb-shaped inputs each produce one plain message, no crash, no app wedge, batch continues — the decoder runs inside the §2.12 isolation boundary (P4), a hanging/crashing engine fails **one** item (`EngineCrash`/`EngineHang`, §2.8). Backed by the explicit §6.4.5 decompression-bomb FIXTURES (svgz bomb, ZIP-bomb-in-OPC DOCX, deeply-nested PDF flate stream).
- [ ] **P9.35** [TEST] Run the in-core detector fuzz validation (the one untrusted-byte path OUTSIDE the §2.12 boundary) — caps fire, XXE disabled · §6.4.2 §1.2 · G48
  > exercise the §1.2 detection layer's coverage-guided `cargo-fuzz` target over `crate::detect`/sniff on a hostile ZIP/OLE2/gzip/svgz/XML corpus (Linux+macOS nightly; per-push = fast `proptest` smoke / saved-crash-corpus replay), asserting **no panic/abort**, the decompression-ratio cap (≤100×) and `MAX_SVGZ_SNIFF` (≤64 KiB) bound **actually fire**, and the XML reader has **DTD/external-entity resolution disabled by construction** (defeats XXE / billion-laughs). Distinct from G26 (the engine-side T1 through the boundary) — libFuzzer is in-process Rust and cannot reach the isolated C/C++ engines.
- [ ] **P9.36** [TEST] Run the engine-subprocess black-box mutational fuzz (radamsa-through-the-isolation-wrapper) as the required scheduled job · §6.4.2 §6.1.4 §2.12 · G42b
  needs: P9.33
  > the §6.1.4/P0.7.15-policy engine-fuzz (G65, named in prose — not a catalogue row): a black-box mutational fuzz of the **real G37-staged SHA-256-verified bundled sidecar** (NOT a debug build) — AFL++ binary-only/QEMU or a `radamsa` harness through the §2.12 wrapper (+ `zzuf` LD_PRELOAD for LibreOffice headless) reusing the §6.4.2 oracles (no-crash-escapes-boundary + no-egress + no-out-of-input-read via G42b). CI-host resource bound via cgroup/`ulimit`/`systemd-run`/`docker --memory` + G56 `timeout-minutes` so a corpus-induced OOM/disk-fill is a contained finding; pre-committed to a REQUIRED **scheduled** (non-PR-blocking) weekly job that FILES AN ISSUE on a boundary-escaping crash. Status recorded in `gate-status.md`.
- [ ] **P9.37** [TEST] Run the T9b corpus sentinels — AutoOpen macro canary not created, WEBSERVICE() no-egress, ImageMagick BMP/SVG-via-MSL sentinel · §6.4.2 §3.5.5 §0.11 · G42 G42b
  needs: P9.32
  > the §7.5/P0.5.9 T9b sentinels exercised under the egress window: a `.docm`/`.xlsm`/`.pptm` AutoOpen canary is NOT created, a `WEBSERVICE()` `.xlsx` produces no egress, and a crafted BMP / SVG-via-MSL ImageMagick sentinel (the densest-CVE family, §3.5.5) neither egresses nor reads out-of-input — exercising the P5–P7 per-engine argv/build controls without re-deciding them.

---

### Threat-map verification (§0.11)

- [ ] **P9.38** [TEST] Verify each §0.11 threat class is exercised by a concrete P9 validation (T1/T6/T7/T8/T9a/T9b/T10/T11) · §0.11 §6.4.2 · G26 G42 G42b G48
  needs: P9.32, P9.34, P9.35, P9.36
  > a traceability check that every §0.11 class with a runtime-exercisable control has a P9 box that exercises it: T1 (engine-side fault-injection through the boundary, P9.34/P9.36 + in-core P9.35); T9a (benign offline, P9.29); T9b (adversarial-egress + read-half + sentinels, P9.32/P9.37); T10 (resource budget, P9.41); T11 (macOS first-TCC-accessor, P9.40); T7 input-side symlink/junction + T8 self-feeding/batch-expansion (P9.39). Complements the static §0.11↔§5 16-class parity (plan-lint check 8, P0.3.5) with the *empirical exercise* — no new threat class, no new isolation.
- [ ] **P9.39** [TEST] Exercise the §2.4.2/§2.4.3 T8 self-feeding / batch-expansion + the T7 INPUT-side symlink/junction case · §6.4.2 §2.4.2 §2.4.3 §0.11 · G31
  > the T8 self-feeding integration case (a freshly-written output in a source folder is invisible to the frozen-set snapshot; outputs landing in a source folder never expand/restart the batch) + the T7 input-side symlink/junction case (a source reached via a link is de-duplicated by resolved identity, never followed onto an unexpected target) — exercising the P2/P3 frozen-set + resolved-identity controls, not re-implementing them.

---

### Performance budgets + `[DEFER: corpus]` empirical validation (§1.10 / §3.9 / §2.12.3)

- [ ] **P9.40** [TEST] Verify the §2.12.3 macOS T11 first-TCC-accessor + privilege-drop-tier-applied per-run regression assertion · §2.12.3 §0.11 · G31 G64
  > the per-run regression assertion that the achieved §2.12.3 privilege-drop tier is actually applied per platform (the cheap-tier floor always; the best-effort tier where it works) + the macOS T11 `stage_for_tcc`-before-spawn behaviour; the per-platform tier-APPLIED assertion is the G31 leg (P0.5.9), the TREND/ratchet is G64 (P9.42).
- [ ] **P9.41** [TEST] Validate + calibrate the §1.10 resource budgets against the §6.4.5 corpus (TooBig ceilings, 1.3× headroom, GIF ~10s cap) · §1.10 §2.8 · G31
  needs: P4.71, P4.72
  > exercise the §1.10 `[DEFER: corpus]` numbers (the design is `[DECIDED]`; only the numbers are empirical) against the P4-built §1.10 estimation engine (P4.71) + its mid-run write-time enforcement (P4.72) — this box CALIBRATES; it does not build the engine: the ~4 GB per-item / ~16 GB aggregate-batch projected-output TooBig ceiling, the memory/handle ceilings, the per-category heuristic constants, the **1.3× headroom margin**, the **~10 s GIF duration cap** — a 1 KB→50 GB intermediate within memory/time budget fails `Failed(TooBig)`, batch continues, scratch returns to ~baseline (the T10 output/scratch-BYTE-budget sub-case, P0.5.9). Record the corpus-calibrated finite starting values.
- [ ] **P9.42** [TEST] Fill the §2.12.3 privilege-drop-tier matrix + drive the G64 decrease-guarded ratchet · §2.12.3 · G64
  needs: P9.40
  > record the achieved §2.12.3 privilege-drop tier per platform in the tracked `privilege-drop-coverage.toml` (the schema homed by P0.7.14), **decrease-guarded** like the coverage floor (a commit lowering an achieved tier fails/escalates; raises are deliberate); owner-decidable required-vs-informational (informational while the matrix fills, required once stable), the flip recorded in `docs/process/gate-status.md` (plan-lint check 23).
- [ ] **P9.43** [TEST] Validate the remaining `[DEFER: corpus]` empirical items — CJK font breadth, engine-ownership pair spikes, OGG/OPUS picture round-trip, deinterlace default, MOV-as-target demand · §6.4.5 §6.4.3 · G31 G32
  needs: P9.23
  > close the scattered `[DEFER: corpus]` validations against the real corpus: the §3.9.3 CJK/RTL font-breadth floor (no tofu from the bundled set), the `MD→PDF` / `RTF→markup` LO-vs-pandoc + pandoc image-policy data-file spikes (§3.5.4), the OGG/OPUS embedded-picture round-trip (§3.5.x), the flagged-interlaced deinterlace default (video.md), and the **video.md MOV-as-target ship-vs-demote decision** (the §9/§6.6 usability-walkthrough corpus check: if the corpus shows no real demand, MOV-as-target is demoted to source-only) — each recorded as a corpus-backed `[DECIDED]` outcome in this plan's notes, not left open. **On a pessimistic outcome the wiring CONSEQUENCE is named, not just recorded:** OGG/OPUS-fails → OGG/OPUS move to the tag-poor list and `audio_tags_dropped` now fires (the **P6.16/P6.29 trigger-map edited**); MOV-target-demoted → drop the MOV cell from the **P6.43 target registration** + add a `docs/demoted-pairs.md` row (`kind=corpus-no-demand`) + the matrix-column update (so the §6.8 governance gate finds the row).
- [ ] **P9.44** [TEST] Validate the [OPEN-E]/[OPEN-A]/[OPEN-C]/[OPEN-F] cross-category to-GIF + extract-audio `[DEFER: corpus]` confirmations against the §6.6 walkthrough · §6.4.5 §6.6 · G31
  needs: P9.43
  > confirm the cross-category.md `[DEFER: corpus]` UX items the §6.6 walkthrough validates: to-GIF **trim** scope ([OPEN-E], leans Basic start+duration), the GIF duration-cap + ceiling numbers ([OPEN-F]), the extract-audio target subset M4A/OGG additions on the MP3+WAV+FLAC floor ([OPEN-A]), and the "no audio track" probe-up-front-vs-offer-then-fail call ([OPEN-C]) — recorded outcomes, not re-opened design. **The wiring CONSEQUENCE of each is named** (not only the decision): [OPEN-A] M4A/OGG-confirmed → the **P6.69 extract-audio additions** subset registration is enabled (and the §3.4 M4A gate P6.54); [OPEN-E]/[OPEN-F] confirmed → the to-GIF trim/cap defaults lock into **P6.58/P6.59**; a demote/adjust on any → the corresponding P6 box edited + a `docs/demoted-pairs.md` row where a pair is dropped.
- [ ] **P9.45** [TEST] Record the headed-E2E + a11y + egress + fidelity validation outcomes in `docs/usability-floor.md` / the pair-status ledger · §6.6 §6.5.2 · G33b
  needs: P9.16, P9.17, P9.42
  > land the human-half evidence the Lane-B gates check: the keyboard-only + readable-contrast + text-size floor (incl. the macOS-contrast gap §6.4.6a leaves) and the SR smoke pass (VoiceOver/NVDA/Orca, §6.6, against the §5.6.1 traversal validated by P9.16) recorded in `docs/usability-floor.md` with `release_line`+`date`; the offline-egress + fidelity validation status reflected in the §6.5.2 pair-status ledger so the §6.7.2 Lane-B staleness/consistency gates have current evidence.

---

### Cross-phase reconciliation (the deferred P9→P4 mechanism `needs:`)

- [ ] **P9.46** [GATE] Wire the deferred P9→P4 mechanism `needs:` edges — §2.12 isolation boundary, §2.12.3 privilege-drop tiers, the egress/fault validations exercise · §2.12 §2.12.3 · G7 G20
  needs: P4.36, P4.15, P4.16, P4.17, P4.18
  > the P9 instance of the cross-phase reconciliation obligation (the master plan-lint forbidden-string check is P4.76): P9 EXERCISES controls earlier phases built, so the boxes that run a P4-built mechanism must carry the edge — the fault-injection / adversarial-egress / fuzz boxes (P9.32/P9.34/P9.36/P9.37) run the decoder through the **P4.36 §2.12 isolation boundary**; the privilege-drop-tier validation (P9.40/P9.42) reads the **P4.15/P4.16/P4.17 best-effort tiers + the P4.18 `privilege-drop-coverage.toml`** the G64 ratchet drives. `needs:` these P4 boxes here so the §6 selection builds the P4 mechanism first (P4 is `[x]` before the loop reaches P9 — the edges must RESOLVE, not dangle; the per-engine §3.5.x controls P9 exercises are P5–P7's, named in each P9 box's prose). No P9 box `>`-note defers a `needs:` with the P4.76-forbidden phrasing.
