import { describe, it, expect } from "vitest";

import type {
  AppFault,
  CollectedSet,
  DestinationResolved,
  OutputPlanPreview,
  RerunPrompt,
  RunResult,
  TargetOffer,
} from "../lib/ipc/commands";

import {
  initialState,
  launchCollectingState,
  transition,
  type Planned,
  type SingleSet,
  type State,
} from "./machine";

// §6.4.6 unit (G15): the §5.2 walking-skeleton state machine (P3.53). The FULL slice reducer lives in
// `machine.ts` (the 2026-07-13 option-① ruling), so EVERY §5.2 transition cell of the slice states is a
// reducer arm — each asserted here as a pure `transition(state, msg)` read-back (test-strategy §0.2), plus the
// no-op behaviour of an invalid (state, msg) pair and the global `app://fault` wildcard from every state.

// ─── §0.6 DTO builders (minimal, valid wire shapes the slice arms consume) ───────────────────────────────
const singleSet = (id = "cs1"): SingleSet => ({
  id,
  instance: "inst-1",
  format: "csv",
  items: [],
  count: 0,
  skipped: [],
  totalBytes: 0,
  rootsDisplay: [],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
});
const collectedSingle = (id = "cs1"): CollectedSet => ({ single: singleSet(id) });
const collectedMixed = (): CollectedSet => ({
  mixed: {
    found: [
      ["jpg", 3],
      ["png", 2],
    ],
  },
});
const collectedUnsupported = (): CollectedSet => ({ unsupported: { detected: "PDF" } });
const collectedUncertain = (): CollectedSet => ({ uncertain: { note: "couldn't tell" } });
const collectedEmpty = (): CollectedSet => ({ empty: { skipped: [] } });
const offer = (): TargetOffer => ({ set: "cs1", targets: [], defaultTarget: { format: "tsv" } });
const preview = (rerun: RerunPrompt | null = null): OutputPlanPreview => ({
  set: "cs1",
  finalDirDisplay: "/out",
  diverted: null,
  rerun,
  preflight: { estTotalOutputBytes: 4096, estTotalScratchBytes: 0, upFrontFail: null },
});
const planned = (rerun: RerunPrompt | null = null, persistedFallback = false): Planned => ({
  set: singleSet(),
  offer: offer(),
  selected: { format: "tsv" },
  options: {},
  destination: "besideSource",
  preview: preview(rerun),
  persistedFallback,
});
const runResult = (): RunResult => ({
  collectedSetId: "cs1",
  runId: "r1",
  items: [],
  totals: { succeeded: 1, failed: 0, cancelled: 0, skipped: 0 },
  cleanupIncomplete: [],
  commonRootDisplay: "/out",
  divertRootDisplay: null,
});
const fault = (): AppFault => ({ kind: "webviewFault", message: "core disconnected" });

// The ten slice states, for the wildcard sweep + the no-op checks.
const targetsSt = (rerun: RerunPrompt | null = null): State => ({
  tag: "targets",
  plan: planned(rerun),
});
const allStates: State[] = [
  { tag: "idle" },
  { tag: "collecting", collectingId: "c1", scanned: null },
  { tag: "confirm", set: singleSet() },
  targetsSt(),
  { tag: "rerunPrompt", plan: planned({ equivalentCount: 2 }), rerun: { equivalentCount: 2 } },
  { tag: "converting", runId: "r1", cancelling: false },
  { tag: "summary", result: runResult() },
  { tag: "mixedDropRefusal", found: [["jpg", 3]] },
  { tag: "unsupported", reason: { kind: "unsupported", detected: "PDF" } },
  { tag: "appFault", fault: fault() },
];

// ─── construction ────────────────────────────────────────────────────────────────────────────────────────
describe("machine construction (§5.2 initial states)", () => {
  it("initialState() is Idle (the plain-launch store-init default)", () => {
    expect(initialState()).toEqual({ tag: "idle" });
  });
  it("launchCollectingState() is Collecting with the mount-drain handle (§7.8.1 launch-with-files)", () => {
    expect(launchCollectingState("c9")).toEqual({
      tag: "collecting",
      collectingId: "c9",
      scanned: null,
    });
  });
});

// ─── state 1: Idle ───────────────────────────────────────────────────────────────────────────────────────
describe("Idle (§5.2 state 1)", () => {
  const idle: State = { tag: "idle" };
  it("startCollecting → Collecting", () => {
    expect(transition(idle, { type: "startCollecting", collectingId: "c1" })).toEqual({
      tag: "collecting",
      collectingId: "c1",
      scanned: null,
    });
  });
  it("pickerCancelled STAYS Idle (its own Msg, never the Empty arm)", () => {
    expect(transition(idle, { type: "pickerCancelled" })).toEqual(idle);
  });
  it("collected(Single) from the launch mount-drain → Confirm", () => {
    expect(transition(idle, { type: "collected", set: collectedSingle() })).toEqual({
      tag: "confirm",
      set: singleSet(),
    });
  });
  it("collected(Empty) from the plain-launch mount-drain STAYS Idle (no files launched)", () => {
    expect(transition(idle, { type: "collected", set: collectedEmpty() })).toEqual(idle);
  });
  it("collected(Mixed) → MixedDropRefusal", () => {
    expect(transition(idle, { type: "collected", set: collectedMixed() })).toEqual({
      tag: "mixedDropRefusal",
      found: [
        ["jpg", 3],
        ["png", 2],
      ],
    });
  });
  it("collected(Unsupported) → Unsupported(unsupported)", () => {
    expect(transition(idle, { type: "collected", set: collectedUnsupported() })).toEqual({
      tag: "unsupported",
      reason: { kind: "unsupported", detected: "PDF" },
    });
  });
});

// ─── state 2: Collecting ─────────────────────────────────────────────────────────────────────────────────
describe("Collecting (§5.2 state 2)", () => {
  const collecting: State = { tag: "collecting", collectingId: "c1", scanned: null };
  it("scanTick updates the live count", () => {
    expect(transition(collecting, { type: "scanTick", scanned: 42 })).toEqual({
      tag: "collecting",
      collectingId: "c1",
      scanned: 42,
    });
  });
  it("collected(Single) → Confirm", () => {
    expect(transition(collecting, { type: "collected", set: collectedSingle() })).toEqual({
      tag: "confirm",
      set: singleSet(),
    });
  });
  it("collected(Empty) from a USER drop → Unsupported(empty) (nothing convertible), NOT Idle", () => {
    expect(transition(collecting, { type: "collected", set: collectedEmpty() })).toEqual({
      tag: "unsupported",
      reason: { kind: "empty", skipped: [] },
    });
  });
  it("collected(Uncertain) → Unsupported(uncertain), carrying the note", () => {
    expect(transition(collecting, { type: "collected", set: collectedUncertain() })).toEqual({
      tag: "unsupported",
      reason: { kind: "uncertain", note: "couldn't tell" },
    });
  });
  it("cancelCollect (Esc) → Idle", () => {
    expect(transition(collecting, { type: "cancelCollect" })).toEqual({ tag: "idle" });
  });
});

// ─── state 3: Confirm ────────────────────────────────────────────────────────────────────────────────────
describe("Confirm (§5.2 state 3)", () => {
  const confirm: State = { tag: "confirm", set: singleSet() };
  it("targetsReady → Targets, selecting the offer's default + threading the set + the §5.8:926 fallback fact forward", () => {
    const next = transition(confirm, {
      type: "targetsReady",
      offer: offer(),
      plan: preview(),
      destination: "besideSource",
      // The C14 hand-off reported a re-validation fallback — the fact must thread into Planned (→ the DestinationBar note).
      persistedFallback: true,
    });
    expect(next).toEqual({
      tag: "targets",
      plan: {
        set: singleSet(),
        offer: offer(),
        selected: { format: "tsv" },
        options: {},
        destination: "besideSource",
        preview: preview(),
        persistedFallback: true,
      },
    });
  });
  it("cancel → Idle", () => {
    expect(transition(confirm, { type: "cancel" })).toEqual({ tag: "idle" });
  });
});

// ─── states 4/5: Targets + Destination ───────────────────────────────────────────────────────────────────
describe("Targets/Destination (§5.2 states 4/5)", () => {
  it("selectTarget updates the selection (the wiring re-fires C4)", () => {
    const next = transition(targetsSt(), { type: "selectTarget", target: { format: "csv" } });
    expect(next).toEqual({ tag: "targets", plan: { ...planned(), selected: { format: "csv" } } });
  });
  it("planResolved refreshes the preview (a debounced C4 re-plan)", () => {
    const refreshed = { ...preview(), finalDirDisplay: "/elsewhere" };
    const next = transition(targetsSt(), { type: "planResolved", plan: refreshed });
    expect(next).toEqual({ tag: "targets", plan: { ...planned(), preview: refreshed } });
  });
  it("destinationResolved refreshes destination + preview but CARRIES rerun through (§2.5.1)", () => {
    // Start with a plan that carries a rerun verdict; C5 must NOT drop it (destination-independent).
    const start = targetsSt({ equivalentCount: 2 });
    const resolved: DestinationResolved = {
      destination: { chosenRoot: "11111111-1111-4111-8111-111111111111" },
      finalDirDisplay: "/chosen",
      diverted: null,
      preflight: { estTotalOutputBytes: 8192, estTotalScratchBytes: 0, upFrontFail: null },
      rerun: null,
    };
    const next = transition(start, { type: "destinationResolved", resolved });
    expect(next).toEqual({
      tag: "targets",
      plan: {
        ...planned({ equivalentCount: 2 }),
        destination: { chosenRoot: "11111111-1111-4111-8111-111111111111" },
        preview: {
          ...preview({ equivalentCount: 2 }),
          finalDirDisplay: "/chosen",
          preflight: { estTotalOutputBytes: 8192, estTotalScratchBytes: 0, upFrontFail: null },
        },
      },
    });
  });
  it("destinationResolved CLEARS persistedFallback — the user actively chose, so the §5.8:926 note no longer applies", () => {
    // Start in a persisted-fallback state (returning user whose saved path failed re-validation → beside-source).
    const start: State = { tag: "targets", plan: planned(null, true) };
    const resolved: DestinationResolved = {
      destination: { chosenRoot: "11111111-1111-4111-8111-111111111111" },
      finalDirDisplay: "/chosen",
      diverted: null,
      preflight: { estTotalOutputBytes: 8192, estTotalScratchBytes: 0, upFrontFail: null },
      rerun: null,
    };
    const next = transition(start, { type: "destinationResolved", resolved });
    expect(next.tag).toBe("targets");
    if (next.tag === "targets") {
      // The stale fallback note must not survive an active Change — else it contradicts the new "will save to …".
      expect(next.plan.persistedFallback).toBe(false);
      expect(next.plan.destination).toEqual({
        chosenRoot: "11111111-1111-4111-8111-111111111111",
      });
    }
  });
  it("convert WITH a rerun verdict → RerunPrompt", () => {
    const next = transition(targetsSt({ equivalentCount: 3 }), { type: "convert" });
    expect(next).toEqual({
      tag: "rerunPrompt",
      plan: planned({ equivalentCount: 3 }),
      rerun: { equivalentCount: 3 },
    });
  });
  it("convert WITHOUT a rerun verdict is a no-op (the wiring fires C6; runStarted drives Converting)", () => {
    const start = targetsSt();
    expect(transition(start, { type: "convert" })).toEqual(start);
  });
  it("runStarted (the no-rerun path) → Converting", () => {
    expect(transition(targetsSt(), { type: "runStarted", runId: "r1" })).toEqual({
      tag: "converting",
      runId: "r1",
      cancelling: false,
    });
  });
  it("back → Confirm, PRESERVING the threaded frozen set (§5.2 row-4 Back)", () => {
    expect(transition(targetsSt(), { type: "back" })).toEqual({ tag: "confirm", set: singleSet() });
  });
  it("cancel (Ctrl/⌘+N from Targets/Destination) → Idle, discarding the set (§5.10 row 1180)", () => {
    expect(transition(targetsSt(), { type: "cancel" })).toEqual({ tag: "idle" });
  });
});

// ─── state 6: RerunPrompt ────────────────────────────────────────────────────────────────────────────────
describe("RerunPrompt (§5.2 state 6)", () => {
  const rerun: State = {
    tag: "rerunPrompt",
    plan: planned({ equivalentCount: 2 }),
    rerun: { equivalentCount: 2 },
  };
  it("runStarted (after the decision fired C6) → Converting", () => {
    expect(transition(rerun, { type: "runStarted", runId: "r2" })).toEqual({
      tag: "converting",
      runId: "r2",
      cancelling: false,
    });
  });
  it("rerunCancel (Esc) → Targets, held plan intact (§5.2 row 6)", () => {
    expect(transition(rerun, { type: "rerunCancel" })).toEqual({
      tag: "targets",
      plan: planned({ equivalentCount: 2 }),
    });
  });
});

// ─── states 7 + 7a: Converting ───────────────────────────────────────────────────────────────────────────
describe("Converting + Cancelling (§5.2 states 7/7a)", () => {
  const converting: State = { tag: "converting", runId: "r1", cancelling: false };
  it("runFinished → Summary", () => {
    expect(transition(converting, { type: "runFinished", result: runResult() })).toEqual({
      tag: "summary",
      result: runResult(),
    });
  });
  it("cancelRun → the 7a Cancelling sub-state", () => {
    expect(transition(converting, { type: "cancelRun" })).toEqual({
      tag: "converting",
      runId: "r1",
      cancelling: true,
    });
  });
  it("a SECOND cancelRun while already cancelling is IGNORED (§5.2 row 7a — no double-cancel)", () => {
    const cancelling: State = { tag: "converting", runId: "r1", cancelling: true };
    expect(transition(cancelling, { type: "cancelRun" })).toEqual(cancelling);
  });
  it("runFinished from 7a → Summary (a partial run)", () => {
    const cancelling: State = { tag: "converting", runId: "r1", cancelling: true };
    expect(transition(cancelling, { type: "runFinished", result: runResult() })).toEqual({
      tag: "summary",
      result: runResult(),
    });
  });
  it("runFault → AppFault (a mid-run backend disconnect, §5.8)", () => {
    expect(transition(converting, { type: "runFault", fault: fault() })).toEqual({
      tag: "appFault",
      fault: fault(),
    });
  });
});

// ─── states 8/9/10/12: terminal + pre-flight + fault ─────────────────────────────────────────────────────
describe("Summary / pre-flight / fault (§5.2 states 8/9/10/12)", () => {
  it("Summary + convertMore → Idle", () => {
    expect(transition({ tag: "summary", result: runResult() }, { type: "convertMore" })).toEqual({
      tag: "idle",
    });
  });
  it("MixedDropRefusal + redrop → Collecting (§5.2 row 9)", () => {
    expect(
      transition(
        { tag: "mixedDropRefusal", found: [["jpg", 3]] },
        { type: "redrop", collectingId: "c2" },
      ),
    ).toEqual({ tag: "collecting", collectingId: "c2", scanned: null });
  });
  it("MixedDropRefusal + dismiss → Idle", () => {
    expect(
      transition({ tag: "mixedDropRefusal", found: [["jpg", 3]] }, { type: "dismiss" }),
    ).toEqual({
      tag: "idle",
    });
  });
  it("Unsupported + dismiss → Idle", () => {
    expect(
      transition(
        { tag: "unsupported", reason: { kind: "unsupported", detected: "PDF" } },
        { type: "dismiss" },
      ),
    ).toEqual({ tag: "idle" });
  });
  it("AppFault + startOver → Idle", () => {
    expect(transition({ tag: "appFault", fault: fault() }, { type: "startOver" })).toEqual({
      tag: "idle",
    });
  });
});

// ─── the app://fault wildcard + invalid-transition no-ops ────────────────────────────────────────────────
describe("app://fault wildcard + no-op invariants (§5.2 state-12 wildcard)", () => {
  it("appFault routes to AppFault from EVERY slice state (the global wildcard edge)", () => {
    for (const state of allStates) {
      expect(transition(state, { type: "appFault", fault: fault() })).toEqual({
        tag: "appFault",
        fault: fault(),
      });
    }
  });
  it("an invalid (state, msg) pair is a no-op — the machine never fabricates a transition", () => {
    // A convert in Idle, a scanTick in Summary, a confirm-cancel in Converting: each returns the state verbatim.
    expect(transition({ tag: "idle" }, { type: "convert" })).toEqual({ tag: "idle" });
    expect(
      transition({ tag: "summary", result: runResult() }, { type: "scanTick", scanned: 1 }),
    ).toEqual({
      tag: "summary",
      result: runResult(),
    });
    expect(
      transition({ tag: "converting", runId: "r1", cancelling: false }, { type: "cancel" }),
    ).toEqual({
      tag: "converting",
      runId: "r1",
      cancelling: false,
    });
  });
});
