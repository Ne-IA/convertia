# P1 — Foundation & Scaffolding

> **The walking skeleton's skeleton.** P1 turns a clean checkout into a buildable,
> bootable, gate-covered ConvertIA: the monorepo + pnpm workspace, the Tauri v2
> shell (Rust core + React 19 / TypeScript-strict / Tailwind / Vite WebView), the
> baseline §0.10 capabilities/CSP, the `src/strings/ui.ts` + `a11y/` module shells,
> the §6.8 governance docs + `.github/` templates, and the §6.7.1 Lane-A CI scaffold
> — so an **empty ConvertIA window boots on Windows, macOS and Linux** and every P1
> commit already runs through the loop + the P0 gates.
>
> Derives from [00-architecture](../spec/00-architecture.md) (§0.4 mechanics, §0.7
> tree, §0.8 pins, §0.10 capabilities/CSP), [06-build-test-release](../spec/06-build-test-release.md)
> (§6.1 build matrix, §6.7.1 Lane A, §6.8 governance), [07-app-shell](../spec/07-app-shell.md)
> (§7.2 boot, §7.6 no-update), [05-ui-ux](../spec/05-ui-ux.md) (§5.1 strings/a11y modules,
> §5.6/§5.7/§5.10). Index: [plan/README.md](README.md). Box format:
> [`_format.md`](_format.md). Conflict order: **SSOT > spec > security/process docs > plan**.
>
> **This is the v0 base — the atomic `[ ]` boxes below.** A multi-round adversarial
> review will deepen, split and complete them afterwards. Boxes are kept as small and
> single-purpose as the spec allows. Box-ids are **phase-scoped, two-segment**
> (`P1.<n>`, 1-based gap-free across the whole phase — the `### ` headings are group
> labels, not box-id segments), with at most one level of sub-boxes (`P1.<n>.<m>`).
>
> **P0 activation targets.** Many P0 boxes carry `> → activated in P1`: their
> enforcement targets (the workspace `Cargo.toml`/`Cargo.lock`, `pnpm-lock.yaml`,
> `tauri.conf.json`/capabilities/`index.html`, the `strings/ui.ts` keys, the
> codegen output, the cross-platform build matrix) are **scaffolded here**. P1
> boxes that stand those targets up name the P0 gate they satisfy in their `Gnn`
> refs so a later reconciliation pass can match P0's `needs:` against real P1
> box-ids. P0 itself is **not** a `needs:` target of any P1 box — P0 is buildable
> on a clean checkout and is `[x]` before the loop reaches P1.

---

## Monorepo, pnpm workspace & repo skeleton

The §0.7 physical tree exists as an empty-but-wired monorepo with a pinned
toolchain, so every later box has a home and the P0 lockfile/registry gates have a
target. The workspace + lockfiles come first because every language gate
(`cargo-deny`/`clippy`/`tsc`) and every later module sits inside them.

- [ ] **P1.1** [BUILD] Scaffold the §0.7 physical directory tree (`src-tauri/`, `src/`, `tests/`, `scripts/`, `docs/`) · §0.7
  > the canonical on-disk layout from the §0.7 "Physical tree" block — empty directories + `.gitkeep` placeholders where a tree must exist before its files land; the tree mirrors the logical-module decomposition so a later box drops a file into a pre-existing home.
- [ ] **P1.2** [BUILD] Author the root `package.json` + the pnpm workspace definition · §0.7 §0.8 · G18d
  needs: P1.1
  > the repo-root `package.json` (name/private/scripts placeholders) + `pnpm-workspace.yaml` declaring the `src/`-rooted frontend package; pins the pnpm package-manager field to the §0.8 `pnpm@10.13.1` class and sets the `onlyBuiltDependencies` posture the P0 G18d lockdown asserts.
  - [ ] **P1.2.1** [BUILD] Commit the `.npmrc` registry pin + lifecycle-script posture · §0.8 · G18c G18d
    > the committed `.npmrc` pinning the registry (the P0 G18c resolution-URL guard's target) and asserting `enable-pre-post-scripts=false` / no `unsafe-perm`; this is the config the P0.3.8 JS supply-chain skeleton was authored against.
  - [ ] **P1.2.2** [BUILD] Generate + commit the initial `pnpm-lock.yaml` · §0.8 · G18a G18c
    > the first resolved lockfile so the P0 G18a `--frozen-lockfile` + `git diff --exit-code` contract and the G18c resolution-URL guard have a real file to act on (activates P0.4.9 / P0.3.8 for the JS half).
- [ ] **P1.3** [BUILD] Commit `rust-toolchain.toml` pinning the §0.8 stable channel · §0.8 · G24
  > the exact stable channel + components (`rustfmt`, `clippy`, `llvm-tools-preview` for `cargo-llvm-cov`) asserted not-floating — the file the P0.2.1 "asserted not-floating" check + the date-pinned-nightly-for-fuzz wiring reference. (The nightly date-pin for `cargo-fuzz` is P0.2.1-owned; this box pins the stable channel the toolchain builds on.)
- [ ] **P1.4** [BUILD] Author the root `.gitignore` + `.gitattributes` (text/EOL + LFS hooks) · §0.7 · G52 G24a
  > ignore `target/`, `node_modules/`, `dist/`, scratch/build outputs; `.gitattributes` normalises EOL (the P0 G52 editorconfig hygiene companion) and reserves the `filter=lfs` attribute lines the P0 G24a corpus-LFS gate (P0.5.4) keys on; `.gitattributes` joins the L(-1) security-critical-file set.
- [ ] **P1.5** [DOC] Author the root `.editorconfig` for EOL/charset/final-newline · tooling-only
  > the committed `.editorconfig` the P0 G52 `editorconfig-checker` consumes (its config target lands in P1); pure config with no spec-§/gate of its own beyond the P0-built gate that reads it.

---

## Rust core crate & Cargo workspace

The `src-tauri/` Rust crate(s) compile from a clean checkout with the §0.7 module
decomposition as interface-only shells, so `clippy`/`cargo-deny`/the unsafe-policy
gate have real crates to act on (activating P0.4.1/P0.3.6/P0.3.7/P0.4.2).

- [ ] **P1.6** [RUST] Author the Cargo workspace root + the `src-tauri` core member · §0.7 §0.8 · G18a G53
  needs: P1.1
  > the workspace `Cargo.toml` (`[workspace]` members + resolver "2") with the `src-tauri` core crate as the first member; establishes the workspace-member graph the P0 G53 core-crate forbidden-dependency gate (P0.3.7) scopes its bans to, and the closure G18 (P0.3.6) bans the updater/HTTP-client family in.
  - [ ] **P1.6.1** [RUST] Reserve the `convertia-imgworker` workspace member as an empty crate · §3.5.5 §0.7 · G53 G29
    > a compile-only stub member (`fn main`) so the workspace graph carries BOTH first-party crates the P0 G29 `#![deny(unsafe_code)]`-on-every-crate-root check + the G53 core-must-not-link-imgworker-libs rule address from P1; the libvips/libheif link work is P4/P5.
  - [ ] **P1.6.2** [RUST] Reserve an `xtask` workspace member for codegen/coverage bins · §0.4.5 §6.7.1 · G19
    > the `xtask` crate that hosts the §0.4.5 codegen invocation + the §6.7.1 step-4/4a Rust xtask bins; named so the P0 G19 drift-check (P0.3.9) can point at a concrete `cargo xtask codegen` command rather than passing on a stale file via a wrong invocation.
- [ ] **P1.7** [RUST] Generate + commit the initial `Cargo.lock` · §0.8 · G18a G18b
  needs: P1.6
  > the first resolved Rust lockfile so the P0 G18a `--locked`/`git diff --exit-code` contract + the P0.3.6 `cargo vet check`-on-the-initial-`Cargo.lock` exit gate + the `cargo-deny` advisory/license scan have a real lockfile (activates P0.4.9 / the P0.3.6 clean-`cargo vet check` exit for the Rust half).
- [ ] **P1.8** [RUST] Apply the unsafe-policy crate attributes — `#![deny(unsafe_code)]` per first-party crate root · §2.12 §3.5.2 · G29
  needs: P1.6, P1.6.1
  > the crate-root `#![deny(unsafe_code)]` on the core AND `convertia-imgworker` + the single allow-listed FFI module placeholder (`#[allow(unsafe_code)]` appears on exactly one module) — the literal source-level target the P0.4.2 unsafe-policy primary SAST gate (G29) was authored against; FFI module is empty in P1 (filled P4/P5).
- [ ] **P1.9** [RUST] Stand up the §0.7 tier-3 `domain` module shell + the §0.6 identity newtypes · §0.6 · G29
  needs: P1.6
  > `crate::domain` with the §0.6 identity newtypes (`InstanceId`/`RunId`/`CollectedSetId`/`ItemId`/`CollectingId`) as compile-only `specta::Type`-deriving stubs — the lowest tier (depends on nothing); the full §0.6 type set is a P2 pipeline-contract task, so P1 lands only the identity spine the tree needs to compile.
- [ ] **P1.10** [RUST] Stand up the §0.7 tier-3 `outcome` module shell (error-taxonomy home) · §2.8 · G29
  needs: P1.9
  > `crate::outcome` (the renamed-from-`error` §0.7 module) as an interface-only home for the §2.8 taxonomy + the §0.4.3 `IpcError`/`ErrorKind` wire mirror; P1 lands the module + an empty placeholder so the tree compiles and §06's drift mechanism has a home — the full catalog/strings are P2/§02.
- [ ] **P1.11** [RUST] Stand up the §0.7 `platform`/`fs_guard`/`run`/`detection`/`engines`/`isolation`/`pool`/`orchestrator`/`ipc` module shells · §0.7 · G29 G9
  needs: P1.6, P1.9
  > one compile-only `mod` per §0.7 logical module with its canonical path (`crate::fs_guard`/`crate::run`/`crate::isolation`/`crate::pool`/`crate::detection`/`crate::engines`/`crate::orchestrator`/`crate::ipc`/`crate::platform`), dependencies pointing strictly downward, so the §0.7 architecture exists as code (not just a tree) and the P0 G9 repo-invariant greps (no `Command::new` outside `crate::isolation`, no `127.0.0.1` outside `#[cfg(test)]`) have their real module boundaries (activates the P0.3.10 invariants (b)/(c)).

---

## Tauri v2 shell & app boot

The Tauri v2 host builds and shows an empty window on all three OS from a clean
checkout (the P1 goal proper), wiring the §0.8 plugins and the §7.2.1 ordered
startup spine as far as the foundation allows.

- [ ] **P1.12** [RUST] Author the `tauri-build` `build.rs` + the §0.4.5 codegen hook seam · §0.4.0 §0.4.5 · G19
  needs: P1.6
  > `src-tauri/build.rs` running `tauri_build::build()` + the optional tauri-specta generation hook seam (the actual `bindings.ts` emission is P1.16); named so the P0 G19 drift framework (P0.3.9) binds to a concrete generated path + command.
- [ ] **P1.13** [RUST] Stand up `main.rs` — the Tauri `Builder`, `tokio` runtime, empty `invoke_handler` + `collect_commands!`/`collect_events!` seam · §0.4.0 §0.4.5 §0.8
  needs: P1.11, P1.12
  > the minimal Tauri v2 entrypoint: the multi-thread `tokio` setup Tauri's async commands run on (§0.8), an empty-but-present `invoke_handler` + the `collect_commands![]`/`collect_events![]` macros (no C-commands yet — those are P2) so the codegen surface exists; the §0.10 capability covers the `main` window so a future command is invokable with no per-command entry.
- [ ] **P1.14** [RUST] Register the §0.8 Tauri plugins in the Builder (single-instance, dialog, store, log, opener) · §0.8 §0.10
  needs: P1.13
  > `tauri_plugin_single_instance::init` / `tauri_plugin_dialog::init` / `tauri_plugin_store` / `tauri_plugin_log` / `tauri_plugin_opener` registered in the Builder — the crates §1.1/§0.4.1/§7.4/§7.5/§7.7 depend on; their WebView grants are §0.10 (dialog/opener are Rust-side-only, NOT WebView capabilities). Wiring only; the handlers that USE them are P2+.
- [ ] **P1.15** [RUST] Stand up the minimal `setup` closure stages the empty window needs (NOT the §7.2.1 ordering) · §7.2.1 §7.2.2
  needs: P1.14
  > the minimal `setup` closure the bootable empty window needs as named-but-mostly-empty stages: single-instance guard (real via the plugin), `InstanceId` + base-path resolution via `app.path()`, and the window-create slot. **P1 does NOT own the §7.2.1 step ORDER** — the §7.2.1 ordered startup-sequence spine (steps 1–8, the engine-presence / exec-permission / scratch-orphan-reclaim / launch-intake / WebView-absent-fault slots) is the **app-shell spine homed in P2's startup-sequence-ordering cluster** per the README P2 scope; P1 lands only the compile-and-boot stages, P2 establishes the ordering, later phases fill the bodies. The §7.2.1 ref is read-only context here (the ordered sequence is P2's box).
  - [ ] **P1.15.1** [RUST] Assert §7.2.2 zero-startup-network as a boot invariant test · §7.2.2 §2.11 · G29
    > a unit/property assertion that the boot path opens no socket (the §7.2.2 observable property + the Lane-A compensating guard for the Lane-B-only egress gate, §6.7.1); pairs with the P0 G29 `std::net` allow-list rule (rule (g)) which is initially empty.
- [ ] **P1.16** [RUST] Create the §7.3.1 main window + show an empty WebView frame · §7.3.1 §0.3.1
  needs: P1.13, P1.20, P1.27
  > the single `main` window from `tauri.conf.json` showing the loaded (empty) React frame — the literal P1 "empty ConvertIA window boots" deliverable; the §0.3.1 WebView-runtime floor (WebView2/WKWebView/WebKitGTK) is relied-on, not bundled.
- [ ] **P1.17** [RUST] Implement the §7.2.4 portable-build executable-permission setup (unix `+x` idempotent) · §7.2.4
  needs: P1.14
  > the `ensure_executable` unix helper (`0o111`-bit set idempotently on each launch) from §7.2.4 — load-bearing for the portable macOS/Linux artifact where extracted sidecars may lack `+x`; Windows is a no-op. P1 lands the helper only (no engines to chmod yet — exercised P4); its slot in the §7.2.1 step-4 sequence is wired by the P2 startup-ordering spine.
- [ ] **P1.18** [RUST] Assert the §7.6.1 no-updater posture by construction · §7.6.1 §7.6 · G47
  needs: P1.14
  > assert `tauri-plugin-updater` is absent from `Cargo.toml`/the Builder and no `updater`/pubkey/endpoint config exists — "its absence is the implementation" (§7.6.1); the structural form is the P0 G47 lint over `tauri.conf.json` (no `updater` block / no `createUpdaterArtifacts`), so this box names what the G47 target must NOT contain.

---

## Tauri config, capabilities & CSP baseline

`tauri.conf.json` + `capabilities/main.json` + `index.html` exist and match the
§0.10 locked allowlist/CSP object exactly — the literal targets the P0 G47
CSP/capability structural lint (P0.3.2) was authored against, flipping it from
fail-open to fail-closed.

- [ ] **P1.19** [BUILD] Author `tauri.conf.json` — bundle identity, window, externalBin/resources slots, minimum-OS floor · §0.3.1 §0.7 §3.3 · G47
  needs: P1.1
  > the base `tauri.conf.json`: app identifier (`dev.ne-ia.convertia`), the §7.3.1 window, empty-but-declared `bundle.externalBin`/`bundle.resources` slots (engines land P4–P7), and the §0.3.1 supported-OS floor knobs (`minimumSystemVersion: "11.0"`, the Windows/Linux floor notes); the file the P0 G47 lint parses.
  - [ ] **P1.19.1** [BUILD] Set `productName: "ConvertIA"` + the §7.3.1 main-window title in `tauri.conf.json` · §7.3.1 §0.3.1 · G47
    > set the case-sensitive `productName: "ConvertIA"` (the case the §6.9.3 rename pass + P9.4.2's `squashfs-root/usr/bin/*` glob + P11.2.2 depend on — `ConvertIA`, NOT `convertia`) and the §7.3.1 main-window title; the field is load-bearing for the AppImage binary name + the Linux/macOS bundle name, not a cosmetic. (The FINAL "ConvertIA"/Ne-IA name itself stays an owner-controlled placeholder per §6.9.3; the slot + the v1 working name are set here.)
  - [ ] **P1.19.2** [BUILD] Stage a bundled PLACEHOLDER icon set wired into `tauri.conf.json → bundle.icon` · §6.9.3 §0.3.1 · G47
    > stage the Tauri-required icon set (`32x32.png` / `128x128.png` / `128x128@2x.png` / `icon.icns` / `icon.ico` / the Windows `Square*Logo.png` set) as a bundled-local **placeholder** under `src-tauri/icons/` and wire `bundle.icon` to it, so the build produces a real installable artifact from P1 on (Tauri fails the bundle with no icon set). The FINAL Ne-IA art is the §6.9.3-deferred owner deliverable swapped in the P8.23-class scope-(ii) pass (mirrors the P8.2 BrandLogo placeholder pattern); P1 lands only the placeholder slot. The G47 structural lint additionally asserts `bundle.icon` is non-empty + `productName` is set (the file the lint parses).
- [ ] **P1.20** [BUILD] Encode the §0.10 locked CSP object in `tauri.conf.json → app.security.csp` · §0.10 · G47
  needs: P1.19
  > the exact §0.10 CSP directives (`default-src`/`script-src` `'self'`; `connect-src 'self' ipc: http://ipc.localhost`; `img-src`/`media-src` NO `asset:`; `object-src`/`frame-src`/`frame-ancestors 'none'`; `base-uri`/`form-action 'self'`; `webrtc 'block'`) — structurally equal per-directive to the locked object the P0 G47 lint asserts against (activates G47's CSP leg, P0.3.2).
- [ ] **P1.21** [BUILD] Author `src-tauri/capabilities/main.json` — the §0.10 deny-by-default allowlist · §0.10 · G47
  needs: P1.19
  > the minimal capability set (`core:default`, `log:default`, `store:default`; NO `fs:`, NO `http:`, NO `shell:allow-execute`, NO `opener:*`, NO `dialog:allow-open`) for the `main` window — the literal allowlist the P0 G47 capability leg asserts (fails any `fs:`/`http:`/`shell`/`opener:`/`dialog:` grant).
- [ ] **P1.22** [BUILD] Assert the three §0.10 release-hardening keys absent/false in `tauri.conf.json` · §0.10 · G47
  needs: P1.19
  > `app.withGlobalTauri` absent/false, `app.security.dangerousDisableAssetCspModification` absent/false/empty, release-profile `devtools` not enabled — the three by-construction T2-widening knobs the P0 G47 lint asserts absent (P0.3.2); P1 lands the conf in the asserted-clean shape.
- [ ] **P1.23** [BUILD] Add the `index.html` shell with the `x-dns-prefetch-control:off` meta · §0.10 · G47
  needs: P1.19
  > the WebView entry `index.html` (Vite mount point) carrying the `<meta http-equiv="x-dns-prefetch-control" content="off">` the §0.10 / P0 G47 lint asserts present — closes the DNS-prefetch side channel CSP alone cannot.
- [ ] **P1.24** [BUILD] Assert no custom URL scheme / no `deep-link` / no file-association in any bundle manifest · §0.10 §7.8.2 · G47
  needs: P1.19
  > no `plugins.deep-link` block and no custom URL-scheme in any `Info.plist`/`.desktop`/`.reg` under `src-tauri/` — the §7.8.2 explicit-negative posture the P0 G47 lint scans for; P1 lands the bundle config in the no-scheme shape (the §7.8 intake funnel itself is P2).

---

## Rust↔TS type-sharing & IPC codegen scaffold

The §0.4.5 tauri-specta codegen pipeline emits the single tracked `bindings.ts`
and CI can prove it non-stale — activating the P0 G19 drift framework (P0.3.9) with
a concrete command + path, even though the C-command surface is empty until P2.

- [ ] **P1.25** [RUST] Wire the §0.4.5 tauri-specta builder + the `collect_types!` registry seam · §0.4.5 §0.6
  needs: P1.9, P1.13
  > the tauri-specta `Builder` configured with `collect_commands!`/`collect_events!`/`collect_types!` (the §0.6 identity types from P1.9 registered so they don't generate as `any`), emitting to the single tracked path — the codegen engine the §06 drift check guards; empty command set in P1 (C1–C13 are P2).
- [ ] **P1.26** [UI] Generate + commit `src/lib/ipc/bindings.ts` at the single §0.7 tracked path · §0.4.5 §0.7 · G19
  needs: P1.25, P1.29
  > run the codegen and commit the generated `src/lib/ipc/bindings.ts` (the frontend's only IPC door) — the concrete file the P0 G19 drift check regenerates + `git diff --exit-code`s; activates G19 (P0.3.9) with a real generated target.
- [ ] **P1.27** [UI] Author the `commands.ts`/`events.ts` typed-façade re-export shells · §5.1 §5.8
  needs: P1.26
  > `src/lib/ipc/commands.ts` + `events.ts` re-exporting the generated `bindings.ts` wrappers — the §5.1 hard-rule seam ("only `src/lib/ipc/**` imports `@tauri-apps/api`"); empty re-exports in P1 (feature code that consumes them is P2+), so the one-IPC-consumer discipline is lint-enforceable from the first commit.
- [ ] **P1.28** [CI] Define the concrete `cargo xtask codegen` invocation for the G19 drift check · §0.4.5 · G19
  needs: P1.6.2, P1.26
  > the named `cargo xtask codegen` command (regenerates `bindings.ts` → the P0 G19 framework calls THIS, not a guessed invocation) so the gate cannot silently pass on a stale file via a wrong command; the §06-owned drift check (authored P0.3.9) binds to it here.

---

## WebView app (React 19 / TS-strict / Tailwind / Vite)

The React 19 / TypeScript-strict / Tailwind / Vite frontend builds and mounts an
empty app inside the WebView — activating the P0 TS gate contract (G5/G6/G13) and
the per-push a11y leg (G33a) against real source.

- [ ] **P1.29** [UI] Author `vite.config.ts` + the §0.3.1 cross-WebView build target · §5.1 §0.3.1
  needs: P1.2
  > the Vite config building the WebView bundle Tauri loads, with the build target set to the §0.3.1 intersection of WebView2/WKWebView/WebKitGTK (no bleeding-edge CSS/JS that drifts); the `dev` server + `build` outputs `tauri dev`/`tauri build` consume.
- [ ] **P1.30** [UI] Author `tsconfig.json` — TypeScript strict, no `any` · §5.1 §0.4.5 · G5 G6 G13
  needs: P1.2
  > the strict `tsconfig.json` (`strict: true`, `noImplicitAny`, the platform no-`any` rule) covering `src/**` incl. the generated `bindings.ts` — the project the P0 TS gates `tsc --noEmit` (G6 diff-scoped / G13 whole-project) act on (activates the P0.4.7 contract for the TS half).
- [ ] **P1.31** [UI] Stand up `main.tsx` + `App.tsx` — React 19 root mount + empty screen-state router shell · §5.1 §5.2
  needs: P1.29, P1.30
  > the React 19 root mount (providers) + an `App.tsx` top-level shell that renders an empty/Idle placeholder — the minimal mounted UI the empty window shows; the full §5.2 state machine is P2/P8, so P1 lands only the router seam.
- [ ] **P1.32** [UI] Author the Tailwind config + `design/tokens.css` token-file shell · §5.1 §5.5 · G9
  needs: P1.29
  > the Tailwind setup + an empty-but-present `design/tokens.css` (CSS custom properties) — the single home for colour tokens the P0 G9 invariant (a) ("no hardcoded colour outside `design/tokens.css`") scopes to (activates P0.3.10 invariant (a)); the real token values are P8 polish.
- [ ] **P1.33** [UI] Author the flat ESLint + stylelint config (incl. project-local no-`any` / `fc.gen()` rules) · §5.1 · G5 G9
  needs: P1.30
  > the flat ESLint config + stylelint carrying the project-local rules the P0.4.7 contract names (no `any`, the §6.4.2 `fc.gen()`-shrink-wrapper rule paired with P0 G9 invariant (f)) — the config the P0 G5 lint leg consumes; activates the eslint half of P0.4.7.
- [ ] **P1.34** [UI] Author the Prettier config + the `prettier --check` posture · §5.1 · G3
  needs: P1.30
  > the committed Prettier config the P0 G3 format mirror (`prettier --check`, no auto-write) runs over the TS/CSS/JSON tree; activates the prettier leg of the P0 format gate.
- [ ] **P1.35** [UI] Wire Vitest + `vitest-axe` with the jsdom environment · §5.1 §6.4.6a · G33a
  needs: P1.31
  > the Vitest config (jsdom env) + `vitest-axe@0.1.0` so the P0 a11y per-push leg (G33a — ARIA/role/focus over the rendered React tree, NOT contrast) has a runner + a rendered tree to scan; activates G33a (the §6.4.6a jsdom leg) against the P1.31 mounted app.
- [ ] **P1.36** [UI] Add the §5.1 lint rule enforcing the single-IPC-consumer boundary · §5.1 · G5
  needs: P1.33, P1.27
  > an ESLint rule (or config restriction) failing any `@tauri-apps/api` import outside `src/lib/ipc/**` — the §5.1 "exactly one IPC consumer" discipline the spec requires be lint-enforceable; runs in the P0 G5 lint leg.

---

## Strings module & a11y module shells

`src/strings/ui.ts` and the `a11y/` shells are established as structural
scaffolding (not deferred) per the README P1 scope — activating the P0 G57
English-only / string-ownership lint against a real `strings/ui.ts`.

- [ ] **P1.37** [UI] Stand up `src/strings/ui.ts` — the flat English UI-chrome string table · §5.7 · G57
  needs: P1.31
  > the `strings/ui.ts` flat English key→value table (§5.7 ownership split: UI-chrome strings here, conversion-outcome strings owned by §02) — the module the P0 G57 lint asserts every key resolves to a non-empty English value over; activates G57 (P0.4.6) against a real target. No i18n runtime is added (the §5.7 by-construction Principle-11 enforcement).
  - [ ] **P1.37.1** [UI] Add the fixed-text `idle_reassurance` string key · §5.7 · G57
    > the §5.7 named key with its `[DECIDED]` fixed text `"All conversion happens locally, on your machine — nothing is ever uploaded."` — given concrete-string treatment so the P0 G57 lint / drift check covers it (it is not free-form prose).
- [ ] **P1.38** [UI] Assert no i18n-runtime / locale-switch import ships (Principle-11 by construction) · §5.7 §6.10 · G57
  needs: P1.37
  > the by-construction half of §5.7: no i18n framework / locale-negotiation / `Accept-Language`-driven selection is a dependency — the P0 G57 lint's "fail on any locale-switch/i18n-runtime import" leg; P1 establishes the no-i18n posture the gate enforces.
- [ ] **P1.39** [UI] Stand up `src/a11y/announcer.ts` — the §5.6 ARIA-live announcement helper shell · §5.6
  needs: P1.31
  > the `announcer.ts` interface-only helper (an ARIA-live region announcer) the §5.6 screen-reader path + later focus/announce wiring consume — established here as structural scaffolding per the README P1 scope; the per-component wiring is P4/P8.
- [ ] **P1.40** [UI] Stand up `src/a11y/keymap.ts` — the §5.10 canonical accelerator table shell · §5.10
  needs: P1.31
  > the `keymap.ts` single-source accelerator table (the §5.10 canonical map with `CmdOrCtrl` modifier handling) as a typed, mostly-empty table P5–P10 components reference rather than re-declaring shortcuts — established now so §5.10's "single source" rule holds from the first component.

---

## Governance docs, README & `.github/` templates

The §6.8 governance set + README download/trust skeleton + `.github/` templates
exist from the first commit (they gate contribution and have no build dependency);
the release-blocking governance-completeness GATE is P10, but the DOCS are authored
here.

- [ ] **P1.41** [DOC] Author `LICENSE` — MIT with the collective copyright notice · §6.8
  > MIT + the header `Copyright (c) 2026 Ne-IA and ConvertIA contributors` (inbound=outbound, no assignment) per the §6.8 table; the release gate (present + name matches §6.9 clearance) is P10.
- [ ] **P1.42** [DOC] Author `CONTRIBUTING.md` — inbound=outbound, no-CLA, optional DCO, the stated quality bar · §6.8
  > the §6.8 content: inbound=outbound under MIT, no CLA, optional `Signed-off-by` (requested not required), the inbound-warranty clause, how to run the §6.7.1 lanes, and the quality bar stated directly (no `any`/no `// TODO`/no `console.log` in prod/no inline CSS/production-ready) — NOT by reference to the private `CLAUDE.md`.
- [ ] **P1.43** [DOC] Author `CODE_OF_CONDUCT.md` — Contributor-Covenant-class + enforcement contact · §6.8
  > a standard CoC with the SECURITY/maintainer enforcement contact per the §6.8 table.
- [ ] **P1.44** [DOC] Author `SECURITY.md` — private-advisory channel, untrusted-decoder scope, no-SLA posture · §6.8 §0.11 · G51
  > private vulnerability reporting (GitHub private advisories + contact), the scope statement (ConvertIA opens untrusted files through third-party decoders) referencing the §0.11 threat map + §2.12 isolation, the no-SLA best-effort patch posture, and how a reporter includes a redacted (§7.5) log repro; the public-prose typo gate (P0 G51) covers it. The §0.11 map back-reference is back-filled by P4's threat-map assembly (plan README fill-note) — P1 authors `SECURITY.md` with the reference present.
- [ ] **P1.45** [DOC] Author `PRIVACY.md` — offline restatement of §2.11 + the cloud-sync caveat · §6.8 §2.11 · G51
  > the plain-language §2.11 restatement (fully offline, no network/telemetry/accounts/update-phone-home; the only network is the user-initiated open-project-page) + the OneDrive/iCloud/Dropbox cloud-sync caveat per the §6.8 table; G51 typo-covered.
- [ ] **P1.46** [DOC] Author `TRADEMARK.md` — name/logo grant boundary + nominative use · §6.8 · G51
  > the §6.8 content: MIT covers code not the "ConvertIA" name / Ne-IA logo; forks must rename and may not use the logo; nominative-use guidelines. G51 typo-covered; the §6.9 name-clearance GATE is P10.
- [ ] **P1.47** [DOC] Author the `README.md` download/trust skeleton + per-platform prerequisite notes · §6.8 §6.2.4 §0.3.1 · G51
  > the README skeleton: what it is, canonical-GitHub-Releases-only download, the verify-your-hash recipe SLOT (the literal `minisign -Vm SHA256SUMS -p docs/minisign.pub` recipe is authored/filled in P10 §6.2.4), as-is/no-warranty + best-effort posture, the §0.3.1 supported-OS floor, the unsigned-build first-launch note, and the Windows portable-zip WebView2 + Linux AppImage `libfuse2` prerequisite notes (§6.2.4). Skeleton now; release-gated completeness is P10.
- [ ] **P1.48** [DOC] Author the `NOTICE` + `THIRD-PARTY-LICENSES.txt` generated-file placeholders · §6.8 §6.3.2
  > the placeholder NOTICE / THIRD-PARTY-LICENSES files (marked generated-from-`engines.lock`+SBOM, never hand-drifted) so the §6.8 set is structurally complete from P1; per-engine rows are populated P5–P7 and finalized P10 (the release-blocking completeness gate is P10).
- [ ] **P1.49** [CI] Author the `.github/` issue templates (default new-format requests to Future-Ideas-Parked) · §6.8
  > the issue templates defaulting new-format/feature requests to **Future Ideas (Parked)** per the SSOT inclusion test (§6.8 `.github/` policy row); a `.github/` config change, gate-clean (actionlint/zizmor over any embedded workflow is the P0 G49/G50 plane).
- [ ] **P1.50** [CI] Author the `.github/` PR template (DCO/quality-bar reference) + private-advisory config · §6.8
  > the PR template referencing the DCO/quality bar + the private-advisory config wired to `SECURITY.md` (`.github/SECURITY` advisory routing) per the §6.8 table.

---

## Lane-A CI scaffold (per-push validation on `main`)

The §6.7.1 Lane-A per-push pipeline runs on every push to `main`, wiring the
lint/format/type-check/compile-sanity/audit steps + the 3-OS build into the P0 CI
skeleton (P0.2.4) and binding the now-real language gates to the P1-scaffolded
toolchain. Data-dependent guards (bijection §6.4.3a, defaults-registry §1.6) are
ADDED by the phase that produces their input — NOT here.

- [ ] **P1.51** [CI] Author the Lane-A workflow shell wired into the P0 L4 skeleton · §6.7.1 · G25 G49 G56
  needs: P1.55
  > the `.github/workflows/` Lane-A workflow (push-on-`main` + fork-PR) plugged into the P0.2.4 clean-checkout matrix slot: top-level `permissions: contents: read`, per-job `timeout-minutes`, per-push `concurrency: {group, cancel-in-progress: true}`, SHA-pinned actions — the empty heavy-gate slots P0 left for P1 to fill; actionlint/zizmor-clean (P0 G49/G50).
- [ ] **P1.52** [CI] Wire the §6.7.1 step-1 lint/format leg (rustfmt/clippy/eslint/tsc/prettier/yamllint) · §6.7.1 · G3 G4 G5 G6 G14
  needs: P1.51
  > the Lane-A lint/format step: `cargo fmt --check`, `cargo clippy -D warnings` (+ the no-panic-sloppiness deny set), `eslint` + `tsc --noEmit`, `prettier --check`, `yamllint` — the P0 language gates (G3/G4/G5/G6, full at G14) now bound to the P1-scaffolded Rust crate + TS project (activates the CI wiring-point of P0.4.1 / P0.4.7).
- [ ] **P1.53** [CI] Wire the §6.7.1 step-2 Rust↔TS type-drift check · §6.7.1 §0.4.5 · G19
  needs: P1.51, P1.28
  > the Lane-A step running `cargo xtask codegen` + `git diff --exit-code` on `bindings.ts` (the P1.28 invocation) — fails on stale generated types; the concrete activation of the P0 G19 framework (P0.3.9).
- [ ] **P1.54** [CI] Wire the §6.7.1 step-3 unit + property + fault-injection test leg (Rust + Vitest) · §6.7.1 §6.4.1 · G27 G28
  needs: P1.51
  > the fast engine-light test leg (`cargo test` + Vitest) feeding the P0 coverage floors (G27 per-domain, G28 ≥80% diff) which were created at 0% in P0 and begin enforcing as P1 code lands; activates the coverage gate (P0.4.8) for the foundation crates.
- [ ] **P1.55** [CI] Add `dependabot.yml` coverage for the P1-scaffolded ecosystems (github-actions, cargo, npm) · §6.7.2 · G56
  > extend/confirm `dependabot.yml` covers github-actions + cargo + npm now that the `Cargo.toml`/`package.json` exist — the presence the P0 G56 sub-assertion (P0.2.6) asserts; the pip ecosystem is the gate-tooling `requirements-ci.txt` (P0-owned).
- [ ] **P1.56** [CI] Wire the §6.7.1 step-4b automated-a11y (jsdom) leg · §6.7.1 §6.4.6a · G33a
  needs: P1.51, P1.35
  > the Lane-A `vitest-axe` jsdom step asserting ARIA-role/state validity + focus-order (NOT contrast — that is Lane-B, §6.4.6a) over the rendered React tree; activates the P0 G33a per-push leg with the P1.35 runner.
- [ ] **P1.57** [CI] Wire the §6.7.1 Principle-11 English-only lint leg · §6.7.1 §6.10 · G57
  needs: P1.51, P1.38
  > the Lane-A step running the P0 G57 English-only / string-ownership lint over `strings/ui.ts` (every key non-empty English; no i18n-runtime import) — activates G57 (P0.4.6) against the P1.37 module.
- [ ] **P1.58** [CI] Wire the §6.7.1 step-5 compile-sanity 3-OS matrix (`cargo check` + debug `tauri build`) · §6.7.1 §6.1.4 · G30
  needs: P1.51, P1.16
  > the Win/macOS/Linux matrix running `cargo check` / a debug `tauri build` to catch platform-specific breakage early (no full corpus run) — the literal "empty window boots on 3 OS from clean checkout" CI proof; activates the P0 G30 cross-platform build-matrix contract (P0.4.10) for the debug shell (the universal-`lipo` sidecar leg binds when engines land in P4).
- [ ] **P1.59** [CI] Wire the §6.7.1 step-6 `cargo audit` + `cargo deny` supply-chain leg · §6.7.1 §6.3.4 · G17 G18 G18a G18b G53
  needs: P1.51, P1.7
  > the Lane-A advisory + license + bans + lockfile-integrity leg: `cargo audit` (plain, no `--locked`) + `cargo deny check` over the real `Cargo.lock` (P1.7) — activates the P0 `deny.toml`/`cargo-vet` skeleton (P0.3.6), the lockfile-integrity contract (P0.4.9), and the core-crate forbidden-dep gate (P0.3.7 G53) against the P1 workspace graph.
- [ ] **P1.60** [CI] Wire the JS-tree supply-chain Lane-A leg (resolution-URL + lifecycle-script + frontend license) · §6.7.1 §6.3.4 · G18c G18d G36b
  needs: P1.51, P1.2.2
  > the Lane-A step asserting the P0 G18c resolution-URL guard, G18d `onlyBuiltDependencies` lockdown, and G36b frontend GPL/AGPL deny over the real `pnpm-lock.yaml` (P1.2.2) — activates the P0.3.8 JS supply-chain config against the committed pnpm graph.
- [ ] **P1.61** [CI] Record the Lane-A required-status-check set for the §6.7.1 G56a branch-protection assertion · §6.7.1 · G56a
  needs: P1.52, P1.53, P1.54, P1.56, P1.57, P1.58, P1.59, P1.60
  > enumerate the Lane-A jobs that must be required status checks on `main` (the set the P0 G56a branch-protection config assertion, P0.2.8, queries the ruleset API for) — so a red Lane-A actually blocks; the §6.7.1 single-branch direct-to-`main` enforcement made real now that Lane-A jobs exist.
