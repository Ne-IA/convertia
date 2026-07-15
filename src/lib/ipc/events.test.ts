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
  pickForIntake,
  startConversionRun,
  subscribeAppEvents,
  subscribeNativeDragDrop,
} from "./events";

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
  });

  const handlerFor = (event: string) => listen.mock.calls.find((call) => call[0] === event)?.[1];

  it("registers exactly the three app:// events on mount (the §0.4.2 closed set)", async () => {
    await subscribeAppEvents();
    expect(new Set(listen.mock.calls.map((call) => call[0]))).toEqual(
      new Set(["app://intake", "app://fault", "app://close-requested"]),
    );
    expect(listen).toHaveBeenCalledTimes(3);
  });

  // [Test-Change: P3.77/P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] app://intake is a PAYLOAD-LESS nudge
  // (the core-owned-path ruling retired `IntakePayload`), so the listener issues the args-less C1 `drain_intake`
  // drain (P3.78 — no `paths`/`drainPending`), the same drain the mount issues — never a payload-carrying ingest.
  it("app://intake issues the payload-less drain (C1 drain_intake with a fresh collectingId)", async () => {
    await subscribeAppEvents();
    handlerFor("app://intake")?.({ payload: null });
    await Promise.resolve(); // let the fire-and-forget C1 drain land
    expect(invoke).toHaveBeenCalledWith(
      "drain_intake",
      expect.objectContaining({
        collectingId: expect.any(String),
      }),
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
