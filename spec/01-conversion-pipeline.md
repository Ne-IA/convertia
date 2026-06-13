# 01 — Conversion Pipeline (platform-independent core)

> The canonical, engine-agnostic core: how an item goes from "dropped" to
> "converted output", independent of any specific format or OS. (00.5 is a
> navigational map; **this file owns the pipeline**.)
> Origin: SSOT *How It Feels to Use*, *Recognize files by content*, *Fail clearly*.

## 1.1 Input intake
- Drag-and-drop, file picker, keyboard; folder drop. **All intake paths arrive as
  real FS paths in Rust** — via Tauri **native file-drop** (not HTML5 DnD, §0.4),
  the picker/dialog, **and OS launch entry points** (Open-with / launch args /
  macOS open-doc / Windows argv / Linux `%F`, posture owned by §7.8). **Folder
  recursion runs in Rust** (ignore hidden/system files). The complete set of
  entry points feeds one place that builds the **frozen source set** (snapshot;
  §2.4), so the freeze point is exhaustive incl. the launch-time / second-instance
  hand-off (§7.1). _(expand)_

## 1.2 Content-based format detection
- Magic-byte / content sniffing strategy; detection result model; confidence;
  the misnamed-file and no-extension cases; "detected but unsupported" and
  "uncertain/conflicting" outcomes (SSOT *Recognize files by content*).
- **Security note:** detection is the **first code touching untrusted bytes** —
  state where it runs (in-core vs isolated) and whether header-only sniffing is
  inside or outside the §2.12 isolation boundary. _(expand)_

## 1.3 Batch grouping & the pre-flight rule
- Grouping key = **individual user-facing format** (not category, not codec
  subtype). Same-format-only v1 batch; mixed-drop pre-flight refusal (hard
  reject, list found formats + counts). Resolved-identity de-duplication (§2.3).
  _(expand)_

## 1.4 Collected-set summary & confirm gate
- Produces the **collected-summary payload** (detected format + counts, e.g.
  "48 JPG files") and the mandatory pre-convert **confirm gate** (SSOT How It
  Feels 3). Backend payload here; UI state in §5.2. _(expand)_

## 1.5 Target resolution
- From detected source type → the offered target set (incl. cross-category
  outputs as targets of the source); the **one fixed pre-highlighted default per
  source** (general rule lives here; per-source defaults marked in the 04
  matrices). **One target applies to the whole batch.** Owns the general "a
  multi-category format (e.g. PDF) is one detected type → de-duplicated union of
  targets" rule. _(expand)_

## 1.6 Options model — **owner of the generic option-declaration model**
- Owns the **generic** model: option types, basic-vs-"Advanced", and the
  **no-decision defaulting** rule. Concrete per-pair option lists and default
  **values** live in 04 (per-source) and are **not restated** here. May own a
  consolidated defaults registry the DoD "no required choices" gate verifies
  against. _(expand)_

## 1.7 Engine-invocation model — **generic owner (incl. cancellation/kill)**
- The engine-agnostic subprocess **lifecycle**: spawn → progress channel →
  **cancellation/kill mechanism** (process-group kill vs cooperative; Windows has
  no SIGTERM; ordering so a mid-flight engine dies cleanly while §2.6 cleanup and
  §2.1 no-partial hold) → timeout/hang → exit-code → mapping to the §2.8 error
  taxonomy. **Sole owner of cancellation**; §0.9/§1.11/§5.8 reference it. Runs
  **through the §2.12 isolation wrapper**. Per-engine concrete argument
  construction lives in §3.5 (not restated here). _(expand)_

## 1.8 Output planning
- Computes the `OutputPlan` (resolve destination, re-create relative subtree,
  per-location divert) **before** the write, applying the rules owned by §2.7.
  The §2.1 atomic write consumes this plan. _(expand)_

## 1.9 Job & batch lifecycle
- States: `Pending → Running → Succeeded | Failed | Cancelled | Skipped`.
- Queue semantics, ordering, per-item isolation, mid-run skip vs pre-flight
  refusal. _(expand)_

## 1.10 Resource pre-flight & budgets `[OPEN]`
- The model the SSOT Boundary Note delegates: up-front **output/temp size
  estimation** per pair (+ headroom margin, per-category heuristic), the
  up-front-vs-mid-run fail decision, the **"too big" threshold**, memory/handle
  ceilings (concurrency degree owned by §0.9 — referenced here), and large-list
  (thousands of recursively-collected files) handling + UI virtualization (§5).
  Feeds §1.8 / §2.6 / §2.8 / §2.14. _(expand)_

## 1.11 Progress & cancellation
- Real per-item progress (not indeterminate); aggregate batch progress;
  cancellation surfaced here, **mechanism owned by §1.7**; fast-fail for "too big
  / doomed for space" (§1.10). _(expand)_

## 1.12 End-of-batch summary
- Per-item success/failure, reasons, output locations; fully-failed = clear
  failure; mapping output → source. _(expand)_
