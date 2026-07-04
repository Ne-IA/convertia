import { describe, it, expect } from "vitest";

import type { EngineHealth } from "../lib/ipc/commands";

import { selectUnavailableTargets, useAppStore, type AppStore } from "./store";

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
