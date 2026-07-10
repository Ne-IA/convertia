import { describe, it, expect, afterEach } from "vitest";

import type { ConversionEvent, EngineHealth, RunResult } from "../lib/ipc/commands";

import { reduceConvertEvent, selectUnavailableTargets, useAppStore, type AppStore } from "./store";

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

// §6.4.6 unit (G15): the P2.120 `applyConvertEvent` progress reducer (§5.8 / §0.4.2). `reduceConvertEvent` is
// the pure `(state, event) → changed slice` behind the store action, tested directly for every ConversionEvent
// variant; one integration test drives the live store action. [Build-Session-Entscheidung: P2.120]
describe("reduceConvertEvent (P2.120 §5.8 ConversionEvent reducer)", () => {
  // A clean data baseline independent of test order (the pure reducer never mutates the store).
  const pristine = (): AppStore => ({
    ...useAppStore.getState(),
    progress: {},
    pendingVideoReencodeNote: null,
  });

  it("clears pendingVideoReencodeNote on runStarted when willReencode is false (§5.8 lossless remux path)", () => {
    const state: AppStore = { ...pristine(), pendingVideoReencodeNote: "will re-encode video" };
    const event: ConversionEvent = {
      type: "runStarted",
      data: { runId: "r1", totalItems: 2, willReencode: false },
    };
    expect(reduceConvertEvent(state, event)).toEqual({ pendingVideoReencodeNote: null });
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
    expect(reduceConvertEvent(state, event)).toEqual({ pendingVideoReencodeNote: null });
  });

  it("keeps pendingVideoReencodeNote on runStarted when willReencode is true (worst-case note stands)", () => {
    const state: AppStore = { ...pristine(), pendingVideoReencodeNote: "will re-encode video" };
    const event: ConversionEvent = {
      type: "runStarted",
      data: { runId: "r1", totalItems: 2, willReencode: true },
    };
    // keep = the reducer returns a slice that does not touch the field.
    expect(reduceConvertEvent(state, event)).toEqual({});
  });

  it("adds a determinate no-fraction row on itemStarted", () => {
    const event: ConversionEvent = {
      type: "itemStarted",
      data: { runId: "r1", itemId: 1, sourceDisplay: "/a.csv", target: { format: "tsv" } },
    };
    expect(reduceConvertEvent(pristine(), event)).toEqual({
      progress: { 1: { fraction: null, done: false } },
    });
  });

  it("upserts the live fraction on itemProgress, merging with prior rows", () => {
    const state: AppStore = { ...pristine(), progress: { 0: { fraction: 1, done: true } } };
    const event: ConversionEvent = {
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: 0.5, stage: "encoding" },
    };
    expect(reduceConvertEvent(state, event)).toEqual({
      progress: { 0: { fraction: 1, done: true }, 1: { fraction: 0.5, done: false } },
    });
  });

  it("preserves a null (indeterminate) fraction on itemProgress (§1.11 LibreOffice)", () => {
    const event: ConversionEvent = {
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: null, stage: "decoding" },
    };
    expect(reduceConvertEvent(pristine(), event)).toEqual({
      progress: { 1: { fraction: null, done: false } },
    });
  });

  it("marks the row done on itemFinished (any terminal outcome)", () => {
    const event: ConversionEvent = {
      type: "itemFinished",
      data: { runId: "r1", itemId: 1, outcome: "cancelled" },
    };
    expect(reduceConvertEvent(pristine(), event)).toEqual({
      progress: { 1: { fraction: 1, done: true } },
    });
  });

  it("makes no store change on batchProgress (the §1.11 aggregate bar has no P2 store field)", () => {
    const event: ConversionEvent = {
      type: "batchProgress",
      data: { runId: "r1", done: 1, total: 2 },
    };
    expect(reduceConvertEvent(pristine(), event)).toEqual({});
  });

  it("makes no store change on runFinished (the §1.12 Summary is the P3.53 machine + C8)", () => {
    const runResult: RunResult = {
      collectedSetId: "cs1",
      runId: "r1",
      items: [],
      totals: { succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
      cleanupIncomplete: [],
      commonRootDisplay: "/out",
      divertRootDisplay: null,
    };
    const event: ConversionEvent = { type: "runFinished", data: runResult };
    expect(reduceConvertEvent(pristine(), event)).toEqual({});
  });
});

describe("applyConvertEvent (P2.120 live store action)", () => {
  afterEach(() => {
    // reset the singleton's reducer-touched fields so tests stay isolated.
    useAppStore.setState({ progress: {}, pendingVideoReencodeNote: null });
  });

  it("drives the live store — an itemProgress tick lands in the progress map", () => {
    useAppStore.getState().applyConvertEvent({
      type: "itemProgress",
      data: { runId: "r1", itemId: 1, fraction: 0.25, stage: "writing" },
    });
    expect(useAppStore.getState().progress).toEqual({ 1: { fraction: 0.25, done: false } });
  });
});
