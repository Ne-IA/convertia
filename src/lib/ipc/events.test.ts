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

import { useAppStore } from "../../state/store";

import {
  drainPendingIntake,
  ingestFromDrop,
  ingestFromIntakeEvent,
  startConversionRun,
  subscribeAppEvents,
  subscribeNativeDragDrop,
} from "./events";

describe("drainPendingIntake (§7.8.1 first-launch drain)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("re-calls C1 ingest_paths with no paths + drainPending true + a fresh collectingId", async () => {
    await drainPendingIntake();
    expect(invoke).toHaveBeenCalledTimes(1);
    // §7.8.1 / §0.4.1: a drain sends empty paths (drainPending ⊻ paths) + drainPending=true; the origin is
    // ignored by the Rust drain (stored origin wins) but passed as the launchArg placeholder.
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({
        paths: [],
        origin: "launchArg",
        drainPending: true,
        collectingId: expect.any(String),
      }),
    );
  });

  it("mints a fresh collectingId per drain (no reuse across calls)", async () => {
    await drainPendingIntake();
    await drainPendingIntake();
    const ids = invoke.mock.calls.map((call) => call[1].collectingId);
    expect(new Set(ids).size).toBe(2);
  });
});

describe("subscribeAppEvents (P2.120 §5.8 three app:// listeners)", () => {
  beforeEach(() => {
    listen.mockReset();
    invoke.mockReset();
    listen.mockImplementation(() => Promise.resolve(() => {}));
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  const handlerFor = (event: string) => listen.mock.calls.find((call) => call[0] === event)?.[1];

  it("registers exactly the three app:// events on mount (the §0.4.2 closed set)", async () => {
    await subscribeAppEvents();
    expect(new Set(listen.mock.calls.map((call) => call[0]))).toEqual(
      new Set(["app://intake", "app://fault", "app://close-requested"]),
    );
    expect(listen).toHaveBeenCalledTimes(3);
  });

  it("app://intake re-enters intake via C1 ingest_paths with the event's paths/origin + drainPending null", async () => {
    await subscribeAppEvents();
    handlerFor("app://intake")?.({ payload: { paths: ["/x.png"], origin: "secondInstance" } });
    await Promise.resolve(); // let the fire-and-forget C1 call land
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({ paths: ["/x.png"], origin: "secondInstance", drainPending: null }),
    );
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

describe("startConversionRun (P2.120 §5.8 Channel<ConversionEvent> → store)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue("run-1");
    channels.length = 0;
    useAppStore.setState({ progress: {} });
  });
  afterEach(() => {
    useAppStore.setState({ progress: {} });
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
    // the run-scoped Channel routes an itemProgress tick straight into the live store.
    channels[channels.length - 1]?.onmessage?.({
      type: "itemProgress",
      data: { runId: "run-1", itemId: 1, fraction: 0.5, stage: "encoding" },
    });
    expect(useAppStore.getState().progress).toEqual({ 1: { fraction: 0.5, done: false } });
  });
});

describe("ingestFromIntakeEvent (P2.120 app://intake → C1)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("calls C1 ingest_paths with the payload's paths/origin, a fresh collectingId, drainPending null", async () => {
    await ingestFromIntakeEvent({ paths: ["/a.png", "/b.png"], origin: "launchArg" });
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({
        paths: ["/a.png", "/b.png"],
        origin: "launchArg",
        drainPending: null,
        collectingId: expect.any(String),
      }),
    );
  });
});

describe("subscribeNativeDragDrop (P2.121 §5.4 native file-drop)", () => {
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

  it("on drop: clears drag-active and hands the paths to C1 with origin 'drop' + drainPending null", async () => {
    const onDragActiveChange = vi.fn();
    await subscribeNativeDragDrop({ onDragActiveChange });
    handler()?.({ payload: { type: "drop", paths: ["/a.png", "/b.png"] } });
    await Promise.resolve(); // let the fire-and-forget C1 call land
    expect(onDragActiveChange).toHaveBeenLastCalledWith(false);
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({ paths: ["/a.png", "/b.png"], origin: "drop", drainPending: null }),
    );
  });

  it("de-dups the dropped paths by set before C1 (§5.4 — native events can duplicate)", async () => {
    await subscribeNativeDragDrop();
    handler()?.({ payload: { type: "drop", paths: ["/a.png", "/a.png", "/b.png"] } });
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({ paths: ["/a.png", "/b.png"] }),
    );
  });

  it("a drop with no onDragActiveChange handler still ingests (the visual seam is inert in P2)", async () => {
    await subscribeNativeDragDrop();
    expect(() => handler()?.({ payload: { type: "enter", paths: ["/a"] } })).not.toThrow();
    handler()?.({ payload: { type: "drop", paths: ["/a.png"] } });
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({ origin: "drop" }),
    );
  });

  it("returns the unlisten from onDragDropEvent", async () => {
    const unlisten = vi.fn();
    onDragDropEvent.mockResolvedValue(unlisten);
    const cleanup = await subscribeNativeDragDrop();
    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

describe("ingestFromDrop (P2.121 drop → C1)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("calls C1 ingest_paths with origin 'drop', a fresh collectingId, drainPending null, an onScan Channel", async () => {
    await ingestFromDrop(["/x.png"]);
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({
        paths: ["/x.png"],
        origin: "drop",
        drainPending: null,
        collectingId: expect.any(String),
      }),
    );
  });
});
