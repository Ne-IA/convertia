import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// §6.4.6 unit (G15): the §5.8 IPC subscription helpers (P2.61 drain + P2.120 async model). Mock the §0.4.5
// IPC transport — the generated `bindings.ts` (re-exported by ./commands) calls `invoke` from
// @tauri-apps/api/core, the helpers construct a `Channel`, and P2.120's `app://` listeners call `listen` from
// @tauri-apps/api/event — so every wrapper runs with no Tauri runtime and we read back the EXACT arguments +
// fire events through the mocked Channel/listeners. [Build-Session-Entscheidung: P2.120]
const invoke = vi.fn<(cmd: string, args: Record<string, unknown>) => Promise<unknown>>();
const listen =
  vi.fn<(event: string, handler: (e: { payload: unknown }) => void) => Promise<() => void>>();

// P2.121: the §5.4 native window drag-drop event. The mock records the handler so a test can fire each
// DragDropEvent phase (enter/over/leave/drop) and assert the drag-active visual + the drop→C1 intake.
type DragPayload =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };
const onDragDropEvent =
  vi.fn<(handler: (e: { payload: DragPayload }) => void) => Promise<() => void>>();

// The mock `Channel` records instances + carries `onmessage`, so a test can fire a `ConversionEvent` through
// the `start_conversion` progress Channel and assert it reaches the store's `applyConvertEvent`. Hoisted
// because it is referenced eagerly in the `vi.mock` factory (unlike the lazily-called `invoke`/`listen`).
const { channels, MockChannel } = vi.hoisted(() => {
  const channels: { onmessage: ((msg: unknown) => void) | null }[] = [];
  class MockChannel {
    onmessage: ((msg: unknown) => void) | null = null;
    constructor() {
      channels.push(this);
    }
  }
  return { channels, MockChannel };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: Record<string, unknown>) => invoke(cmd, args),
  Channel: MockChannel,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => listen(event, handler),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: (handler: (e: { payload: DragPayload }) => void) => onDragDropEvent(handler),
  }),
}));

import type { Planned, SingleSet } from "../../state/machine";
import { initialState } from "../../state/machine";
import { useAppStore } from "../../state/store";

import {
  advanceToTargets,
  cancelConversionRun,
  cancelIntakeCollect,
  consumeIntakeNudge,
  consumeMountDrain,
  drainPendingIntake,
  pickAndSetDestination,
  pickForIntake,
  replanOutput,
  runConversion,
  startConversionRun,
  subscribeAppEvents,
  subscribeNativeDragDrop,
} from "./events";

// §1.4 CollectedSet::Single / §1.5 TargetOffer / §1.8 OutputPlanPreview fixtures for the P3.55 consumption +
// advance tests. `singleSet` is typed (it seats the §5.2 `confirm` state in setState); the offer/plan are
// loose (only ever mocked `invoke` return values, `unknown`, and the machine STORES them without validating).
const singleSet: SingleSet = {
  id: "cs1",
  instance: "inst-1",
  format: "csv",
  items: [],
  count: 1,
  skipped: [],
  totalBytes: 10,
  rootsDisplay: ["/drop"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};
const targetOffer = { set: "cs1", targets: [], defaultTarget: { format: "tsv" } };
const outputPlan = {
  set: "cs1",
  finalDirDisplay: "/drop",
  diverted: null,
  rerun: null,
  preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
};

// A typed §5.2 `targets`-state `Planned` (the held FormatPicker + DestinationBar plan) for the P3.56 re-plan /
// change-destination / convert façade tests — these dispatch `planResolved` / `destinationResolved` / `runStarted`,
// all of which the machine only takes from `targets`, so the store must be seated in a real `targets` state.
const plannedFixture: Planned = {
  set: singleSet,
  offer: { set: "cs1", targets: [], defaultTarget: { format: "tsv" } },
  selected: { format: "tsv" },
  options: {},
  destination: "besideSource",
  preview: {
    set: "cs1",
    finalDirDisplay: "/drop",
    diverted: null,
    rerun: null,
    preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
  },
  persistedFallback: false,
};

describe("drainPendingIntake (§7.8.1 first-launch drain)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("calls C1 drain_intake with a fresh collectingId + onScan (no paths/origin/drainPending, P3.78)", async () => {
    await drainPendingIntake();
    expect(invoke).toHaveBeenCalledTimes(1);
    // §7.8.1 / §0.4.1 (P3.78): every drain calls the args-less `drain_intake { collectingId, onScan }` — the
    // WebView supplies no path / origin / drain flag; the core drains its `PendingIntake` buffer.
    expect(invoke).toHaveBeenCalledWith(
      "drain_intake",
      expect.objectContaining({
        collectingId: expect.any(String),
      }),
    );
    // [Test-Change: P3.78 — old-obsolete+new-correct, §0.4.1] the retired `paths: []` / `origin: "launchArg"` /
    // `drainPending: true` assertions are gone — those args no longer exist on the wire (C1 sheds them). Pin the
    // EXACT arg set to `{ collectingId, onScan }` so a re-added path arg reddens.
    const args = invoke.mock.calls[0]?.[1] ?? {};
    expect(Object.keys(args).sort()).toEqual(["collectingId", "onScan"]);
  });

  it("mints a fresh collectingId per drain (no reuse across calls)", async () => {
    await drainPendingIntake();
    await drainPendingIntake();
    const ids = invoke.mock.calls.map((call) => call[1].collectingId);
    expect(new Set(ids).size).toBe(2);
  });
});

describe("pickForIntake (§0.4.1 C2a intake picker — the §5.3 DropZone action, P3.54)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
  });

  // C2a opens the native dialog Rust-side and returns () — the picked set completes via the app://intake
  // nudge → C1 drain (subscribeAppEvents), so this call carries only the § 0.6 PickKind and no path (§5.4).
  it.each([["files"], ["folder"]] as const)(
    "calls C2a pick_for_intake with { kind: %s } (files → browse click, folder → choose-folder)",
    async (kind) => {
      await pickForIntake(kind);
      expect(invoke).toHaveBeenCalledTimes(1);
      expect(invoke).toHaveBeenCalledWith("pick_for_intake", { kind });
    },
  );

  it("carries EXACTLY { kind } on the wire — no path / collectingId / onScan (C2a walks nothing, P3.78)", async () => {
    await pickForIntake("files");
    const args = invoke.mock.calls[0]?.[1] ?? {};
    expect(Object.keys(args)).toEqual(["kind"]);
  });
});

describe("subscribeAppEvents (P2.120 §5.8 three app:// listeners)", () => {
  beforeEach(() => {
    listen.mockReset();
    invoke.mockReset();
    listen.mockImplementation(() => Promise.resolve(() => {}));
    invoke.mockResolvedValue({ empty: { skipped: [] } });
    useAppStore.setState({ machine: { tag: "idle" } });
  });

  const handlerFor = (event: string) => listen.mock.calls.find((call) => call[0] === event)?.[1];

  it("registers exactly the three app:// events on mount (the §0.4.2 closed set)", async () => {
    await subscribeAppEvents();
    expect(new Set(listen.mock.calls.map((call) => call[0]))).toEqual(
      new Set(["app://intake", "app://fault", "app://close-requested"]),
    );
    expect(listen).toHaveBeenCalledTimes(3);
  });

  // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] The app://intake listener now CONSUMES the nudge
  // (`consumeIntakeNudge`) rather than firing the bare `drainPendingIntake` (P3.77): from Idle it enters
  // Collecting + drains + routes the result. The obsolete pin was "issues the drain"; the correct pin is the
  // consumption (enters Collecting synchronously, then drains).
  it("app://intake from Idle CONSUMES the nudge — enters Collecting then drains (§5.8)", async () => {
    await subscribeAppEvents();
    handlerFor("app://intake")?.({ payload: null });
    // consumeIntakeNudge synchronously dispatches startCollecting (Idle → Collecting) BEFORE the async drain.
    expect(useAppStore.getState().machine.tag).toBe("collecting");
    await Promise.resolve(); // let the fire-and-forget C1 drain land
    expect(invoke).toHaveBeenCalledWith(
      "drain_intake",
      expect.objectContaining({ collectingId: expect.any(String) }),
    );
  });

  it("app://intake in a non-Idle state is a no-op — no drain, machine unchanged (§5.4 slice guard)", async () => {
    useAppStore.setState({
      machine: { tag: "converting", runId: "r1", cancelling: false, set: singleSet },
    });
    await subscribeAppEvents();
    handlerFor("app://intake")?.({ payload: null });
    await Promise.resolve();
    expect(useAppStore.getState().machine.tag).toBe("converting");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("app://fault + app://close-requested route to the typed handlers when supplied", async () => {
    const onFault = vi.fn();
    const onCloseRequested = vi.fn();
    await subscribeAppEvents({ onFault, onCloseRequested });
    const fault = { kind: "engineMissing", message: "An engine is missing." };
    handlerFor("app://fault")?.({ payload: fault });
    handlerFor("app://close-requested")?.({ payload: null });
    expect(onFault).toHaveBeenCalledWith(fault);
    expect(onCloseRequested).toHaveBeenCalledTimes(1);
  });

  it("a leaked app://fault/close-requested is a no-op when no handler is supplied (the P2 inert seam)", async () => {
    await subscribeAppEvents();
    expect(() => {
      handlerFor("app://fault")?.({ payload: { kind: "webviewFault", message: "x" } });
      handlerFor("app://close-requested")?.({ payload: null });
    }).not.toThrow();
  });

  it("the returned cleanup drops all three listeners", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation(() => Promise.resolve(unlisten));
    const cleanup = await subscribeAppEvents();
    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(3);
  });
});

describe("consumeMountDrain (§5.8 mount-drain consumption, P3.55)", () => {
  beforeEach(() => {
    invoke.mockReset();
    useAppStore.setState({ machine: { tag: "idle" } });
  });

  it("drains + routes a launch-with-files Single set → Confirm, from Idle (no Collecting transit)", async () => {
    invoke.mockResolvedValue({ single: singleSet });
    await consumeMountDrain();
    expect(invoke).toHaveBeenCalledWith(
      "drain_intake",
      expect.objectContaining({ collectingId: expect.any(String) }),
    );
    expect(useAppStore.getState().machine.tag).toBe("confirm");
  });

  it("a plain-launch Empty STAYS Idle — never Unsupported (the mount-drain asymmetry)", async () => {
    invoke.mockResolvedValue({ empty: { skipped: [] } });
    await consumeMountDrain();
    expect(useAppStore.getState().machine.tag).toBe("idle");
  });
});

describe("consumeIntakeNudge (§5.8 nudge consumption, P3.55)", () => {
  beforeEach(() => {
    invoke.mockReset();
    channels.length = 0;
    useAppStore.setState({ machine: { tag: "idle" } });
  });

  it("from Idle: enters Collecting synchronously, drains, routes a Single set → Confirm", async () => {
    invoke.mockResolvedValue({ single: singleSet });
    const pending = consumeIntakeNudge();
    expect(useAppStore.getState().machine.tag).toBe("collecting");
    await pending;
    expect(useAppStore.getState().machine.tag).toBe("confirm");
  });

  it("a drop's Empty from Collecting → Unsupported (the emptyStaysIdle=false arm)", async () => {
    invoke.mockResolvedValue({ empty: { skipped: [] } });
    await consumeIntakeNudge();
    expect(useAppStore.getState().machine.tag).toBe("unsupported");
  });

  it("routes onScan ticks to the Collecting count (scanTick) over the command-scoped Channel", async () => {
    let resolveDrain: (value: unknown) => void = () => undefined;
    invoke.mockImplementation(
      () =>
        new Promise<unknown>((resolve) => {
          resolveDrain = resolve;
        }),
    );
    const pending = consumeIntakeNudge();
    channels[channels.length - 1]?.onmessage?.({ scanned: 7 });
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("collecting");
    if (machine.tag === "collecting") {
      expect(machine.scanned).toBe(7);
    }
    resolveDrain({ empty: { skipped: [] } });
    await pending;
  });

  it("a nudge in a non-Idle state is a no-op — no drain, buffer preserved (§5.4 slice guard)", async () => {
    useAppStore.setState({
      machine: { tag: "converting", runId: "r1", cancelling: false, set: singleSet },
    });
    await consumeIntakeNudge();
    expect(invoke).not.toHaveBeenCalled();
    expect(useAppStore.getState().machine.tag).toBe("converting");
  });

  it("drops a STALE drain result — an older walk never routes into a newer walk (the collectingId guard)", async () => {
    // Sonnet review: a cancel-then-immediately-redrop can let drain-1 resolve AFTER a NEW walk (walk-2) started.
    // Without the guard, drain-1's `collected(single)` would route from walk-2's Collecting → Confirm(stale).
    let resolveDrain: (value: unknown) => void = () => undefined;
    invoke.mockImplementation(
      () =>
        new Promise<unknown>((resolve) => {
          resolveDrain = resolve;
        }),
    );
    const pending = consumeIntakeNudge(); // walk-1: startCollecting(id1) → Collecting; drain-1 pending
    expect(useAppStore.getState().machine.tag).toBe("collecting");
    // A newer walk supersedes walk-1 (the redrop) while drain-1 is still pending.
    useAppStore.setState({ machine: { tag: "collecting", collectingId: "walk-2", scanned: null } });
    resolveDrain({ single: singleSet }); // drain-1 resolves with a real (now STALE) set
    await pending;
    // The guard drops it: the machine is STILL walk-2's Collecting, NOT Confirm(stale set).
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("collecting");
    if (machine.tag === "collecting") {
      expect(machine.collectingId).toBe("walk-2");
    }
  });
});

describe("cancelIntakeCollect (§5.10 C13 cancel-collect, P3.55)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
    useAppStore.setState({ machine: { tag: "collecting", collectingId: "c1", scanned: 3 } });
  });

  it("advances to Idle then trips C13 cancel_ingest for the walk (§1.1)", async () => {
    await cancelIntakeCollect("c1");
    expect(useAppStore.getState().machine.tag).toBe("idle");
    expect(invoke).toHaveBeenCalledWith("cancel_ingest", { collectingId: "c1" });
  });
});

describe("advanceToTargets (§5.8 Confirm → Targets, P3.55 → the P3.56 persisted-destination hand-off)", () => {
  beforeEach(() => {
    invoke.mockReset();
    useAppStore.setState({ machine: { tag: "confirm", set: singleSet } });
  });

  // Route the three planning commands the advance fires: C14 get_initial_destination (the persisted hand-off), then
  // C3 get_targets + C4 plan_output. `initial` sets the C14 return so each test drives one InitialDestination arm.
  const mockPlanning = (initial: unknown) => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "get_initial_destination") {
        return Promise.resolve(initial);
      }
      if (cmd === "get_targets") {
        return Promise.resolve(targetOffer);
      }
      if (cmd === "plan_output") {
        return Promise.resolve(outputPlan);
      }
      return Promise.reject(new Error(`unexpected ${cmd}`));
    });
  };

  it("C14 besideSource → C4 gets besideSource, targetsReady with persistedFallback=false → Targets", async () => {
    mockPlanning("besideSource");
    await advanceToTargets("cs1");
    expect(invoke.mock.calls.map((call) => call[0])).toContain("get_initial_destination");
    expect(invoke).toHaveBeenCalledWith("get_targets", { collectedSetId: "cs1" });
    expect(invoke).toHaveBeenCalledWith(
      "plan_output",
      expect.objectContaining({
        collectedSetId: "cs1",
        target: { format: "tsv" },
        destination: "besideSource",
      }),
    );
    // [Test-Change: P3.56 — old-obsolete+new-correct, §5.8] the single old "fires C3+C4 → Targets" advanceToTargets
    // test is re-cut into three C14-arm tests (besideSource/chosenRoot/fallback) as the advance now runs the C14
    // persisted-destination hand-off first; the `→ Targets` assertion is preserved here.
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("targets");
    if (machine.tag === "targets") {
      expect(machine.plan.persistedFallback).toBe(false);
    }
  });

  it("C14 chosenRoot(id) → C4 gets ChosenRoot(id) (no path on the wire), persistedFallback=false", async () => {
    mockPlanning({ chosenRoot: { destination: "dest-9", display: "/saved" } });
    await advanceToTargets("cs1");
    expect(invoke).toHaveBeenCalledWith(
      "plan_output",
      expect.objectContaining({ destination: { chosenRoot: "dest-9" } }),
    );
    const machine = useAppStore.getState().machine;
    if (machine.tag === "targets") {
      expect(machine.plan.destination).toEqual({ chosenRoot: "dest-9" });
      expect(machine.plan.persistedFallback).toBe(false);
    }
  });

  it("C14 fallback → C4 gets besideSource AND persistedFallback=TRUE (the §5.8:926 fallback-note fact)", async () => {
    mockPlanning("fallback");
    await advanceToTargets("cs1");
    expect(invoke).toHaveBeenCalledWith(
      "plan_output",
      expect.objectContaining({ destination: "besideSource" }),
    );
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("targets");
    if (machine.tag === "targets") {
      // The structural fallback fact survives into Planned — the DestinationBar renders the passive note even
      // though the resolved destination is beside-source (only the resolver knew the persisted path failed).
      expect(machine.plan.persistedFallback).toBe(true);
    }
  });

  it("re-throws a C3/C4 rejection (→ the §7.5.1 global bridge) leaving the machine in Confirm", async () => {
    invoke.mockRejectedValue({ kind: "internalError", message: "stale set" });
    await expect(advanceToTargets("cs1")).rejects.toBeDefined();
    expect(useAppStore.getState().machine.tag).toBe("confirm");
  });
});

describe("replanOutput (§5.8 Targets re-plan, P3.56)", () => {
  beforeEach(() => {
    invoke.mockReset();
    useAppStore.setState({ machine: { tag: "targets", plan: plannedFixture } });
  });

  it("fires C4 plan_output for the (set, target, options, destination) then dispatches planResolved", async () => {
    const refreshed = { ...outputPlan, finalDirDisplay: "/re-planned" };
    invoke.mockResolvedValue(refreshed);
    await replanOutput("cs1", { format: "tsv" }, {}, "besideSource");
    expect(invoke).toHaveBeenCalledWith(
      "plan_output",
      expect.objectContaining({
        collectedSetId: "cs1",
        target: { format: "tsv" },
        destination: "besideSource",
      }),
    );
    // planResolved folds the refreshed preview into the held plan (the machine stays in Targets).
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("targets");
    if (machine.tag === "targets") {
      expect(machine.plan.preview.finalDirDisplay).toBe("/re-planned");
    }
  });
});

describe("pickAndSetDestination (§5.4/§5.8 Change destination, P3.56)", () => {
  beforeEach(() => {
    invoke.mockReset();
    useAppStore.setState({ machine: { tag: "targets", plan: plannedFixture } });
  });

  const destinationResolved = {
    destination: { chosenRoot: "dest-1" },
    finalDirDisplay: "/picked",
    diverted: null,
    preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
    rerun: null,
  };

  it("on a real pick: fires C2b then C5 with ChosenRoot(id) and dispatches destinationResolved", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "pick_destination") {
        return Promise.resolve({ destination: "dest-1", display: "/picked" });
      }
      if (cmd === "set_destination") {
        return Promise.resolve(destinationResolved);
      }
      return Promise.reject(new Error(`unexpected ${cmd}`));
    });
    await pickAndSetDestination("cs1", { format: "tsv" }, {});
    expect(invoke.mock.calls.map((call) => call[0])).toContain("pick_destination");
    // C5 carries the picked root as the ChosenRoot(DestinationId) wire choice (no path on the wire, §2.10.1).
    expect(invoke).toHaveBeenCalledWith(
      "set_destination",
      expect.objectContaining({
        collectedSetId: "cs1",
        target: { format: "tsv" },
        destination: { chosenRoot: "dest-1" },
      }),
    );
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("targets");
    if (machine.tag === "targets") {
      expect(machine.plan.destination).toEqual({ chosenRoot: "dest-1" });
      expect(machine.plan.preview.finalDirDisplay).toBe("/picked");
    }
  });

  it("a cancelled pick (null) is a no-op — no C5 fires, the held destination is unchanged (§5.4)", async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === "pick_destination" ? Promise.resolve(null) : Promise.reject(new Error("no C5")),
    );
    await pickAndSetDestination("cs1", { format: "tsv" }, {});
    expect(invoke).toHaveBeenCalledTimes(1); // only C2b — no C5
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain("set_destination");
    const machine = useAppStore.getState().machine;
    if (machine.tag === "targets") {
      expect(machine.plan.destination).toBe("besideSource");
    }
  });
});

describe("runConversion (§5.2 Convert → C6, P3.56)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue("run-7");
    channels.length = 0;
    useAppStore.setState({ machine: { tag: "targets", plan: plannedFixture } });
  });

  it("fires C6 start_conversion and dispatches runStarted → Converting", async () => {
    await runConversion("cs1", { format: "tsv" }, {}, "besideSource", "skip");
    expect(invoke).toHaveBeenCalledWith(
      "start_conversion",
      expect.objectContaining({
        collectedSetId: "cs1",
        destination: "besideSource",
        rerunDecision: "skip",
      }),
    );
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("converting");
    if (machine.tag === "converting") {
      expect(machine.runId).toBe("run-7");
    }
  });

  it("re-throws a C6 rejection (→ the §7.5.1 global bridge) leaving the machine in Targets", async () => {
    invoke.mockRejectedValueOnce(new Error("ipc drop"));
    await expect(
      runConversion("cs1", { format: "tsv" }, {}, "besideSource", "skip"),
    ).rejects.toBeDefined();
    expect(useAppStore.getState().machine.tag).toBe("targets");
  });
});

describe("startConversionRun (P2.120 §5.8 Channel<ConversionEvent> → store)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue("run-1");
    channels.length = 0;
    useAppStore.setState({ progress: {} });
  });
  afterEach(() => {
    useAppStore.setState({ progress: {}, batchProgress: null, machine: initialState() });
  });

  it("fires C6 start_conversion and routes Channel events into the store's applyConvertEvent", async () => {
    const runId = await startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip");
    expect(runId).toBe("run-1");
    expect(invoke).toHaveBeenCalledWith(
      "start_conversion",
      expect.objectContaining({
        collectedSetId: "cs1",
        destination: "besideSource",
        rerunDecision: "skip",
      }),
    );
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] the run-scoped Channel routes ItemStarted then an
    // itemProgress tick into the live store; the row is the richer P3.58 shape (merged over ItemStarted),
    // superseding the old bare `{ fraction, done }`.
    const channel = channels[channels.length - 1];
    channel?.onmessage?.({
      type: "itemStarted",
      data: { runId: "run-1", itemId: 1, sourceDisplay: "/a.csv", target: { format: "tsv" } },
    });
    channel?.onmessage?.({
      type: "itemProgress",
      data: { runId: "run-1", itemId: 1, fraction: 0.5, stage: "encoding" },
    });
    // [Test-Change: P3.58 — old-obsolete+new-correct, §5.8] the richer P3.58 row (merged over ItemStarted)
    // supersedes the old bare `{ 1: { fraction: 0.5, done: false } }`.
    expect(useAppStore.getState().progress).toEqual({
      1: { sourceDisplay: "/a.csv", status: "running", fraction: 0.5, reason: null },
    });
  });

  it("dispatches the machine runFinished on a terminal RunFinished event → Summary (P3.58 transition out of Converting)", async () => {
    // §5.8: the onmessage handler ALSO drives the machine Converting → Summary on RunFinished (the store reducer
    // holds no RunResult; the machine carries it). Seat the machine in Converting so `fromConverting` takes it.
    useAppStore.setState({
      machine: { tag: "converting", runId: "run-1", cancelling: false, set: singleSet },
    });
    await startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip");
    const runResult = {
      collectedSetId: "cs1",
      runId: "run-1",
      items: [],
      totals: { succeeded: 1, failed: 0, cancelled: 0, skipped: 0 },
      cleanupIncomplete: [],
      commonRootDisplay: "/out",
      divertRootDisplay: null,
    };
    channels[channels.length - 1]?.onmessage?.({ type: "runFinished", data: runResult });
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("summary");
    if (machine.tag === "summary") {
      expect(machine.result).toEqual(runResult);
    }
  });

  it("signals onRunFault + re-throws on an OPAQUE rejection (P2.124 §5.8 core-panic / IPC-drop)", async () => {
    // A bare Error (no `kind`) is the "rejects unexpectedly (core panic, IPC drop)" case → app-level fault.
    // The seam NOTIFIES (→ AppFault state 12) AND re-throws — it never swallows the failure (P2.124).
    const fault = new Error("ipc drop");
    invoke.mockRejectedValueOnce(fault);
    const onRunFault = vi.fn();
    await expect(
      startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip", { onRunFault }),
    ).rejects.toBe(fault);
    expect(onRunFault).toHaveBeenCalledTimes(1);
  });

  it("re-throws a structured IpcError WITHOUT firing onRunFault (a business Err is not app-level, §5.8)", async () => {
    // Throw-mode surfaces a Rust `Err(IpcError)` (§0.4.3 — e.g. a stale CollectedSetId) as a structured
    // rejection: the DOCUMENTED error contract, NOT a disconnect. It re-throws for the caller (§5.3
    // CommandError, P3.53) but MUST NOT route to the app-level AppFault state.
    const businessErr = {
      kind: "internalError",
      message: "no such batch",
      path: null,
      residue: null,
    };
    invoke.mockRejectedValueOnce(businessErr);
    const onRunFault = vi.fn();
    await expect(
      startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip", { onRunFault }),
    ).rejects.toBe(businessErr);
    expect(onRunFault).not.toHaveBeenCalled();
  });

  it("does not signal onRunFault on a successful start (only an app-level fault routes to state 12)", async () => {
    const onRunFault = vi.fn();
    await startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip", { onRunFault });
    expect(onRunFault).not.toHaveBeenCalled();
  });

  it("a start_conversion rejection with no onRunFault handler still throws (the P2 inert seam)", async () => {
    const fault = new Error("core panic");
    invoke.mockRejectedValueOnce(fault);
    await expect(
      startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip"),
    ).rejects.toBe(fault);
  });

  // P2.137: adversarial rejection shapes over `isIpcError`'s documented fall-through default (§5.8 / P2.124
  // — "an unknown shape falls through to the app-fault path — the safe default, never a silent misroute").
  // Only a string `kind` marks the documented business `IpcError`; every other shape — including a
  // NUMERIC `kind` — is the opaque core-panic / IPC-drop case: onRunFault fires exactly once AND the
  // rejection re-throws verbatim (the seam notifies, it never swallows).
  it.each([
    ["a plain string", "engine exploded"],
    ["a number", 42],
    ["null", null],
    ["an object whose kind is not a string", { kind: 42 }],
  ] as const)(
    "routes an adversarial rejection shape — %s — to the app-fault path and re-throws (§5.8)",
    async (_label, fault) => {
      invoke.mockRejectedValueOnce(fault);
      const onRunFault = vi.fn();
      await expect(
        startConversionRun("cs1", { format: "tsv" }, {}, "besideSource", "skip", { onRunFault }),
      ).rejects.toBe(fault);
      expect(onRunFault).toHaveBeenCalledTimes(1);
    },
  );
});

describe("cancelConversionRun (§5.2 row 7a / §5.8 Cancel-run round-trip, P3.58)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null); // C7 cancel_run returns Ok(()) → null on the wire
    useAppStore.setState({
      machine: { tag: "converting", runId: "run-1", cancelling: false, set: singleSet },
    });
  });
  afterEach(() => {
    useAppStore.setState({ machine: initialState() });
  });

  it("optimistically enters 7a (dispatch cancelRun) then trips C7 cancel_run for the live runId", async () => {
    await cancelConversionRun("run-1");
    const machine = useAppStore.getState().machine;
    // §5.2 row 7a: still Converting, now `cancelling` (the optimistic dispatch, before the backend confirms).
    expect(machine.tag).toBe("converting");
    if (machine.tag === "converting") {
      expect(machine.cancelling).toBe(true);
    }
    // C7 cancel_run tripped for the live runId (idempotent Ok(()), §0.4.1).
    expect(invoke).toHaveBeenCalledWith("cancel_run", { runId: "run-1" });
  });

  it("a second cancel while already in 7a stays cancelling (the machine's cancelRun arm ignores it, §5.2 row 7a)", async () => {
    await cancelConversionRun("run-1"); // → 7a
    await cancelConversionRun("run-1"); // second cancel
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("converting");
    if (machine.tag === "converting") {
      expect(machine.cancelling).toBe(true);
    }
  });
});

// [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the `ingestFromIntakeEvent` describe is removed with
// the function it exercised: `app://intake` is a payload-less nudge now (the core-owned-path ruling retired
// `IntakePayload`), so there is no payload-carrying intake handler. Its coverage lives in the `subscribeAppEvents`
// "app://intake issues the payload-less drain" test + the `drainPendingIntake` describe above.

describe("subscribeNativeDragDrop (P2.121 §5.4 native drag-active affordance; drop is core-side, P3.77)", () => {
  beforeEach(() => {
    onDragDropEvent.mockReset();
    invoke.mockReset();
    onDragDropEvent.mockResolvedValue(() => {});
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  const handler = () => onDragDropEvent.mock.calls[0]?.[0];

  it("toggles drag-active on enter/over/leave (§5.4 visual affordance only — no C1 call)", async () => {
    const onDragActiveChange = vi.fn();
    await subscribeNativeDragDrop({ onDragActiveChange });
    handler()?.({ payload: { type: "enter", paths: ["/a"] } });
    handler()?.({ payload: { type: "over" } });
    handler()?.({ payload: { type: "leave" } });
    expect(onDragActiveChange.mock.calls).toEqual([[true], [true], [false]]);
    expect(invoke).not.toHaveBeenCalled();
  });

  // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the drop is handled CORE-SIDE now — the WebView
  // clears the affordance on drop but NEVER ingests (a WebView ingest would double-ingest the drop the Rust
  // `WindowEvent::DragDrop` handler already funnelled into `PendingIntake`). The former "drop → C1" test + the
  // frontend de-dup test are retired: no frontend ingest means no frontend de-dup (the backend frozen-set
  // de-dup is the authority, §2.4).
  it("on drop: clears drag-active and does NOT ingest (the drop is core-side, P3.77)", async () => {
    const onDragActiveChange = vi.fn();
    await subscribeNativeDragDrop({ onDragActiveChange });
    handler()?.({ payload: { type: "drop", paths: ["/a.png", "/b.png"] } });
    // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the old drop→C1 `toHaveBeenCalledWith` is gone —
    // the drop is core-side, so the WebView makes no C1 call (asserted by not.toHaveBeenCalled below).
    await Promise.resolve(); // give any (forbidden) fire-and-forget C1 call a chance to land
    expect(onDragActiveChange).toHaveBeenLastCalledWith(false);
    // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] old: expect(invoke).toHaveBeenCalledWith(
    // "ingest_paths", {drop paths}) + a separate de-dups test; the drop is core-side now, so the WebView makes
    // NO C1 call and the frontend de-dup test is retired (the backend frozen-set de-dup is the authority, §2.4).
    expect(invoke).not.toHaveBeenCalled();
  });

  it("a drop with no handler is a silent no-op (no ingest, no throw)", async () => {
    await subscribeNativeDragDrop();
    // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] with the drop core-side, a no-handler drop is a
    // silent no-op: the old enter-precondition + drop→C1 `toHaveBeenCalledWith` become not.toThrow + not-called.
    expect(() => handler()?.({ payload: { type: "drop", paths: ["/a.png"] } })).not.toThrow();
    await Promise.resolve();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("returns the unlisten from onDragDropEvent", async () => {
    const unlisten = vi.fn();
    onDragDropEvent.mockResolvedValue(unlisten);
    const cleanup = await subscribeNativeDragDrop();
    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

// [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the `ingestFromDrop` describe is removed with the
// function it exercised: the native drop is handled core-side (`WindowEvent::DragDrop` → the §7.8.1 funnel →
// `PendingIntake`), so the WebView no longer ingests a drop. Its coverage lives in the `subscribeNativeDragDrop`
// "on drop: does NOT ingest" test + the `drainPendingIntake` describe above.
