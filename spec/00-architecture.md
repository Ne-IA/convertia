# 00 — Architecture

> System architecture and the technical skeleton everything else hangs off.
> Origin: SSOT *Portable, no installation*, *Cross-platform, one product*,
> *Local, private & offline*. **Read together with [07-app-shell](07-app-shell.md)**
> — the process model here depends on its instance/run-identity model.

## 0.1 Goals & constraints recap (from SSOT)
- Portable, install-free, one artifact per platform (Win/macOS/Linux), fully
  offline, local & private, modern-clean UI. _(skeleton — expand)_

## 0.2 Framework choice — Tauri `[DECIDED]`
- Why Tauri vs Electron/Wails (size, system WebView, Rust core, reuse of the
  React/TS/Tailwind stack). Trade-offs and what it commits us to. _(expand)_

## 0.3 High-level architecture
- Two-tier: **Rust core** (backend logic, engine orchestration, filesystem, the
  guarantees) + **React/TS frontend** (UI only).
- Process model: Tauri main process, WebView, and **separate engine subprocesses**.
- Diagram + responsibilities per tier. Decoder isolation is owned by §2.12 — this
  section only references it. _(expand)_

### 0.3.1 WebView runtime variance & supported-OS floor `[OPEN]`
- Tauri uses a **different system WebView per OS** (WebView2 / WKWebView /
  WebKitGTK). Real risk for portable / no-installation / no-runtime-fetch:
  WebView2 absent or old on Windows (bundle-vs-rely decision — but no-network
  forbids downloading it), WebKitGTK distro drift, minimum OS versions. Rendering
  drift across runtimes is a §6.4 test implication; a missing/old WebView at
  startup is a §7.2 / §2.13 startup fault. State the supported-OS floor here.
  _(decide & expand)_

## 0.4 Frontend ↔ backend boundary (IPC) — **single authoritative contract**
- This section is the **one canonical enumeration** of the IPC surface: every
  Tauri **command** (name, request/response payload, error shape, cancellation
  token) and every **event** (progress/streaming channels, payloads). 01 (pipeline)
  and 05.8 (UI) **reference** this, never restate it. _(expand)_
- **Boundary fact — native file-drop:** in a Tauri WebView, HTML5 drag-and-drop
  generally does **not** expose real filesystem paths; intake must use **Tauri
  native file-drop events**, and folder recursion (§1.1) runs in **Rust**, not JS
  (the WebView can't enumerate a directory). This constrains §1.1/§5.4. _(expand)_

### 0.4.1 Rust↔TS type-sharing strategy `[OPEN]`
- Decide: manual mirroring vs codegen (ts-rs / specta / tauri-specta) vs
  JSON-schema. Names the tool, where generated types land (§0.7), and the CI
  drift check (§06). Enforces the platform "no `any`" rule. 05.1 references this.
  _(decide — in open-questions log)_

## 0.5 Conversion pipeline overview (navigational only)
- End-to-end flow map: drop → detect → group/confirm → present targets → plan
  output → convert (queue) → write (guarantees) → summarise. **This is a map;
  01 is the canonical owner of the pipeline.** _(expand)_

## 0.6 Core domain model
- Entities: `DroppedItem`, `DetectedFormat`, `Batch`, `ConversionJob`,
  `Target`, `Engine`, `OutputPlan`, `RunResult`, plus `RunId`/`InstanceId`
  (defined in §7.1). Fields + invariants — incl. the explicit invariant
  **one `Target` per `Batch`** (no per-item targets in v1). _(expand)_

## 0.7 Project layout & logical module decomposition
- **Logical modules** (the architecture, owned here): orchestrator /
  guarantees-fs layer (the reusable home of §2.1/§2.3/§2.7/§2.14 atomicity) /
  engine-registry seam (§3.2 — trait crate?) / IPC handlers / detection. State
  dependency direction so the directory tree doesn't *become* the architecture by
  accident.
- **Physical tree:** Rust crate(s), `src-tauri/`, frontend `src/`,
  engines/sidecar, shared/generated types, tests, build scripts — mapping the
  logical modules onto the tree. _(expand)_

## 0.8 Tech stack & pinned versions
- Rust toolchain, Tauri version, Node/pnpm, React 19, Vite, Tailwind, Vitest,
  key Rust crates. Versioning/pinning policy. _(expand)_

## 0.9 Concurrency, threading & engine-subprocess pool — **owner of the concurrency degree**
- Async runtime; worker pool; the **engine-subprocess pool & contention
  governance**: how many engines run at once (the single concurrency-degree
  number lives here; §1.10 references it for budgets), **per-engine parallelism**
  — note **LibreOffice headless is NOT safely parallel under one user profile**
  (serialize it; parallel instances lock/corrupt — a correctness issue), OS
  handle limits, and the timeout/hang policy parameters (mechanism owned by §1.7).
  Bound to §7.1 (instance/run identity) and §2.6 (temp ownership). _(expand)_

## 0.10 Tauri security boundary — capabilities/permissions allowlist + CSP `[OPEN]`
- Tauri v2's explicit capabilities/permissions allowlist (which commands the
  WebView may call; FS / dialog / **shell-opener** / shell-sidecar scope) plus CSP
  (no remote origins → reinforces "no network"). This is the **WebView half** of
  security; §2.12 is the **subprocess/decoder half**. It enables/limits the §3.5
  sidecar invocation and the §7.7 open-folder/file shell-out. _(expand)_

## 0.11 Security model & threat-surface map
- One assembled map (the pieces are owned elsewhere; this verifies coverage):
  threat classes → owner —
  **untrusted decoder input** → §2.12; **malicious WebView content** → §0.10 (CSP/
  allowlist); **bundled-binary supply chain** → §3.8/§6.3 (SBOM); **open-file
  launch of a fresh artifact** → §7.7; **core panic / app fault** → §2.13;
  **copyleft aggregation boundary** → §3.6. The `SECURITY` policy (§6.8)
  references this map. _(expand)_
