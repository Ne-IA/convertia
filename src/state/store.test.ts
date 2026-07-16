import { describe, it, expect, afterEach } from "vitest";

import type { ConversionEvent, EngineHealth, RunResult } from "../lib/ipc/commands";

import { initialState } from "./machine";
import {
  reduceConvertEvent,
  selectUnavailableTargets,
  useAppStore,
  type AppStore,
  type ItemRow,
} from "./store";

// §6.4.6 unit (G15): the P2.114 `EngineHealth` → `unavailableTargets` store-selector seam (§7.2.3 / §5.1).
// The store holds the FULL cached C12 `EngineHealth`; `selectUnavailableTargets` is the §1.10 read-seam the
// P4.70.2 disable-with-reason FormatPicker tiles bind against. No populate ACTION exists in P2 (the cache is
// filled by the §7.2.3 startup probe, P4.45), so the "backing data present" case is asserted by handing a
// constructed `EngineHealth` straight to the pure selector — exactly the state P4 will write.
// [Build-Session-Entscheidung: P2.114]
describe("selectUnavailableTargets (P2.114 §7.2.3 store-selector seam)", () => {
  it("returns [] at the default store state (engineHealth null — no backing data in P2)", () => {
    expect(selectUnavailableTargets(useAppStore.getState())).toEqual([]);
  });

  it("returns [] whenever engineHealth is null (the default, never undefined)", () => {
    const state: AppStore = { ...useAppStore.getState(), engineHealth: null };
    expect(selectUnavailableTargets(state)).toEqual([]);
  });

  it("returns a REFERENTIALLY STABLE [] across null-state calls (§1.10 — no re-render on unrelated writes)", () => {
    // A fresh `[]` literal per call would defeat zustand's Object.is slice-equality and re-render every
    // `useAppStore(selectUnavailableTargets)` subscriber on unrelated store writes; the stable sentinel
    // keeps the seam referentially stable. This asserts the two null-state reads are the SAME reference.
    const first = selectUnavailableTargets(useAppStore.getState());
    const second = selectUnavailableTargets({ ...useAppStore.getState(), engineHealth: null });
    expect(second).toBe(first);
  });

  it("surfaces EngineHealth.unavailableTargets verbatim when the cache is populated", () => {
    // A representative populated cache: two §3.4 patent-gapped / cross-category targets marked unavailable.
    const health: EngineHealth = {
      engines: [],
      unavailableTargets: [{ format: "heic" }, { op: "extractAudio" }],
      allCriticalOk: true,
    };
    const state: AppStore = { ...useAppStore.getState(), engineHealth: health };
    // The seam surfaces the exact cached vector (identity), so a downstream consumer reads it without a copy.
    expect(selectUnavailableTargets(state)).toBe(health.unavailableTargets);
    expect(selectUnavailableTargets(state)).toEqual([{ format: "heic" }, { op: "extractAudio" }]);
  });

  it("returns the empty vector held in a populated-but-all-available EngineHealth", () => {
    const health: EngineHealth = { engines: [], unavailableTargets: [], allCriticalOk: true };
    const state: AppStore = { ...useAppStore.getState(), engineHealth: health };
    expect(selectUnavailableTargets(state)).toEqual([]);
  });
});

// §6.4.6 unit (G15): the `applyConvertEvent` progress reducer (§5.8 / §0.4.2). `reduceConvertEvent` is the pure
// `(state, event) → changed slice` behind the store action, tested directly for every ConversionEvent variant;
// one integration test drives the live store action. [Build-Session-Entscheidung: P2.120 → P3.58]
//
// [Test-Change: P3.58 — old-obsolete+new-correct, §5.8/§1.11] The P2.120 reducer produced a MINIMAL
// `{ fraction, done }` row and dropped both the `ItemFinished` outcome and `BatchProgress` (its own comments
// deferred those to "the ProgressList box" = P3.58). P3.58 fills those seams, so the row shape is now
// `{ sourceDisplay, status, fraction, reason }`, `runStarted` resets the per-run live progress, and
// `batchProgress` stores the aggregate — the old shape's assertions are genuinely obsolete (superseded by the
// §5.8/§1.11 contract this box implements), the new ones verified against the §0.6 `ItemOutcome`/`BatchProgress`
// wire shapes below.
describe("reduceConvertEvent (§5.8 ConversionEvent reducer)", () => {
  // A clean data baseline independent of test order (the pure reducer never mutates the store).
  const pristine = (): AppStore => ({
    ...useAppStore.getState(),
    progress: {},
    batchProgress: null,
    pendingVideoReencodeNote: null,
  });

  const startedRow = (sourceDisplay: string): ItemRow => ({
    sourceDisplay,
    status: "running",
    fraction: null,
    reason: null,
  });

  it("resets live progress + clears the note on runStarted (willReencode false, §5.8 lossless remux path)", () => {
    const state: AppStore = {
      ...pristine(),
      progress: { 0: startedRow("/old.csv") },
      batchProgress: { done: 1, total: 1 },
      pendingVideoReencodeNote: "will re-encode video",
    };
    const event: ConversionEvent = {
      type: "runStarted",
      data: { runId: "r1", totalItems: 2, willReencode: false },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] a new run now clears the previous run's rows +
    // aggregate AND clears the worst-case note; the old `{ pendingVideoReencodeNote: null }`-only assertion is
    // superseded by the P3.58 per-run reset.
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: {},
      batchProgress: null,
      pendingVideoReencodeNote: null,
    });
  });

  it("treats an absent/undefined willReencode as FALSE — clears the note (§5.8 wire-drift robustness)", () => {
    // §5.8 (05-ui-ux.md, "RunStarted.willReencode consumption"): the bindings.ts type is non-optional
    // `boolean` (the Rust field is `bool`) and the core always emits a definite value — yet the spec
    // mandates the UI "treats absent/`undefined` as `false`". The double cast below is the deliberate
    // §5.8 wire-drift simulation the spec mandates despite the non-optional bindings type (never `any`):
    // an undefined that slipped through the wire must CLEAR the pre-shown worst-case note, exactly like
    // `false`, never keep it. [P2.137]
    const state: AppStore = { ...pristine(), pendingVideoReencodeNote: "will re-encode video" };
    const event: ConversionEvent = {
      type: "runStarted",
      data: { runId: "r1", totalItems: 2, willReencode: undefined as unknown as boolean },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] now also resets progress + aggregate (per-run reset);
    // the old note-only assertion is superseded.
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: {},
      batchProgress: null,
      pendingVideoReencodeNote: null,
    });
  });

  it("resets live progress but KEEPS the note on runStarted when willReencode is true (worst-case stands)", () => {
    const state: AppStore = {
      ...pristine(),
      progress: { 0: startedRow("/old.csv") },
      batchProgress: { done: 1, total: 1 },
      pendingVideoReencodeNote: "will re-encode video",
    };
    const event: ConversionEvent = {
      type: "runStarted",
      data: { runId: "r1", totalItems: 2, willReencode: true },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] keep = the slice resets progress + aggregate but
    // does not touch pendingVideoReencodeNote; the old `{}`-only assertion is superseded by the per-run reset.
    expect(reduceConvertEvent(state, event)).toEqual({ progress: {}, batchProgress: null });
  });

  it("adds a `running` row named by its source display on itemStarted (§1.9 Pending→Running)", () => {
    const event: ConversionEvent = {
      type: "itemStarted",
      data: { runId: "r1", itemId: 1, sourceDisplay: "/a.csv", target: { format: "tsv" } },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8/§1.11] the row is the richer P3.58 shape; the old
    // `{ fraction: null, done: false }` assertion is superseded.
    expect(reduceConvertEvent(pristine(), event)).toEqual({
      progress: { 1: { sourceDisplay: "/a.csv", status: "running", fraction: null, reason: null } },
    });
  });

  it("upserts the live fraction on itemProgress, merging with the started row + preserving prior rows", () => {
    const state: AppStore = {
      ...pristine(),
      progress: {
        0: { sourceDisplay: "/done.csv", status: "succeeded", fraction: 1, reason: null },
        1: startedRow("/b.csv"),
      },
    };
    const event: ConversionEvent = {
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: 0.5, stage: "encoding" },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8/§1.11] row 1 keeps its ItemStarted sourceDisplay +
    // gains the fraction (richer P3.58 shape); row 0 is untouched. The old bare `{ fraction, done }` is superseded.
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: {
        0: { sourceDisplay: "/done.csv", status: "succeeded", fraction: 1, reason: null },
        1: { sourceDisplay: "/b.csv", status: "running", fraction: 0.5, reason: null },
      },
    });
  });

  it("preserves a null (indeterminate) fraction on itemProgress (§1.11 LibreOffice)", () => {
    const state: AppStore = { ...pristine(), progress: { 1: startedRow("/b.csv") } };
    const event: ConversionEvent = {
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: null, stage: "decoding" },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §1.11] indeterminate fraction preserved in the richer
    // P3.58 row; the old `{ fraction: null, done: false }` assertion is superseded.
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: { 1: { sourceDisplay: "/b.csv", status: "running", fraction: null, reason: null } },
    });
  });

  it("maps itemFinished Succeeded → terminal `succeeded` row, bar snapped to full", () => {
    const state: AppStore = {
      ...pristine(),
      progress: { 1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.9, reason: null } },
    };
    const event: ConversionEvent = {
      type: "itemFinished",
      data: { runId: "r1", itemId: 1, outcome: { succeeded: { outputDisplay: "/a.tsv" } } },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8/§1.11] the terminal row now maps the ItemOutcome to
    // a `succeeded` status (the P3.58 outcome fill); the old single `{ fraction: 1, done: true }`-for-any-outcome
    // assertion is superseded by the per-outcome mapping (Succeeded/Failed/Cancelled below).
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: { 1: { sourceDisplay: "/a.csv", status: "succeeded", fraction: 1, reason: null } },
    });
  });

  it("maps itemFinished Failed → terminal `failed` row carrying the verbatim §2.8 IpcError message", () => {
    const state: AppStore = {
      ...pristine(),
      progress: { 1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.4, reason: null } },
    };
    const event: ConversionEvent = {
      type: "itemFinished",
      data: {
        runId: "r1",
        itemId: 1,
        outcome: {
          failed: {
            error: {
              kind: "corrupt",
              message: "This file looks damaged and couldn't be converted.",
              pathDisplay: null,
              residueDisplay: null,
            },
          },
        },
      },
    };
    // The row goes Failed, keeps its last fraction, and carries the §2.8 message VERBATIM (never paraphrased).
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: {
        1: {
          sourceDisplay: "/a.csv",
          status: "failed",
          fraction: 0.4,
          reason: "This file looks damaged and couldn't be converted.",
        },
      },
    });
  });

  it("maps itemFinished Cancelled → terminal `cancelled` row, no reason (§1.11 user-cancelled)", () => {
    const state: AppStore = {
      ...pristine(),
      progress: { 1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.3, reason: null } },
    };
    const event: ConversionEvent = {
      type: "itemFinished",
      data: { runId: "r1", itemId: 1, outcome: "cancelled" },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §1.11] this per-outcome Cancelled mapping supersedes the
    // old single "marks the row done on itemFinished" test's `{ 1: { fraction: 1, done: true } }` assertion.
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: {
        1: { sourceDisplay: "/a.csv", status: "cancelled", fraction: 0.3, reason: null },
      },
    });
  });

  it("maps itemFinished Skipped → terminal `skipped` row (exhaustive completeness; live never emits it, P2.37.4)", () => {
    // §0.4.2 / P2.37.4: no LIVE ItemFinished{Skipped} is emitted (skips are the §1.12 terminal-projection path),
    // but the reducer maps it for exhaustive completeness → the plain "Skipped" chrome label, no reason.
    const state: AppStore = {
      ...pristine(),
      progress: { 1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.1, reason: null } },
    };
    const event: ConversionEvent = {
      type: "itemFinished",
      data: { runId: "r1", itemId: 1, outcome: { skipped: { reason: "alreadyConverted" } } },
    };
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: { 1: { sourceDisplay: "/a.csv", status: "skipped", fraction: 0.1, reason: null } },
    });
  });

  it("stores the aggregate on batchProgress (the §1.11 aggregate-bar seam, P3.58)", () => {
    const event: ConversionEvent = {
      type: "batchProgress",
      data: { runId: "r1", done: 1, total: 2 },
    };
    // [Test-Change: P3.58 — old-obsolete+new-correct, §1.11] batchProgress now STORES the aggregate (the P3.58
    // aggregate seam); the old `.toEqual({})` no-store-effect assertion is superseded.
    expect(reduceConvertEvent(pristine(), event)).toEqual({ batchProgress: { done: 1, total: 2 } });
  });

  it("makes no store change on runFinished (the §1.12 Summary is the machine + C8; RunResult is machine-side)", () => {
    const runResult: RunResult = {
      collectedSetId: "cs1",
      runId: "r1",
      items: [],
      totals: { succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
      cleanupIncomplete: [],
      commonRootDisplay: "/out",
      divertRootDisplay: null,
      summaryLineDisplay: "All 0 files converted.",
    };
    const event: ConversionEvent = { type: "runFinished", data: runResult };
    expect(reduceConvertEvent(pristine(), event)).toEqual({});
  });
});

describe("applyConvertEvent (live store action)", () => {
  afterEach(() => {
    // reset the singleton's reducer-touched fields so tests stay isolated.
    useAppStore.setState({ progress: {}, batchProgress: null, pendingVideoReencodeNote: null });
  });

  it("drives the live store — an itemStarted then itemProgress tick lands in the progress map", () => {
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] the row shape is now the richer P3.58 form; the
    // live tick merges over the ItemStarted row (§1.9 ordering) rather than the old bare `{ fraction, done }`.
    const store = useAppStore.getState();
    store.applyConvertEvent({
      type: "itemStarted",
      data: { runId: "r1", itemId: 1, sourceDisplay: "/a.csv", target: { format: "tsv" } },
    });
    store.applyConvertEvent({
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: 0.25, stage: "writing" },
    });
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] the richer P3.58 row supersedes the old bare
    // `{ 1: { fraction: 0.25, done: false } }` live-store assertion.
    expect(useAppStore.getState().progress).toEqual({
      1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.25, reason: null },
    });
  });

  it("stores the aggregate on a live batchProgress tick (P3.58 aggregate seam)", () => {
    useAppStore.getState().applyConvertEvent({
      type: "batchProgress",
      data: { runId: "r1", done: 2, total: 3 },
    });
    expect(useAppStore.getState().batchProgress).toEqual({ done: 2, total: 3 });
  });
});

// §6.4.6 unit (G15): the P3.53 §5.2 machine store-integration — the store HOLDS `machine: State` and drives it
// through the pure `transition` reducer via `dispatch` (the 2026-07-13 P3.53 ruling). The per-transition logic
// is tested exhaustively in `machine.test.ts`; here we assert the store seam (initial state + dispatch → reducer).
describe("machine dispatch (P3.53 §5.2 store integration)", () => {
  afterEach(() => {
    // Reset the singleton's machine field so tests stay isolated.
    useAppStore.setState({ machine: initialState() });
  });

  it("the store's initial machine state is Idle (§5.2 initialState())", () => {
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("dispatch drives the machine through the pure transition reducer (Idle → Collecting)", () => {
    useAppStore.getState().dispatch({ type: "startCollecting", collectingId: "c1" });
    expect(useAppStore.getState().machine).toEqual({
      tag: "collecting",
      collectingId: "c1",
      scanned: null,
    });
  });
});
