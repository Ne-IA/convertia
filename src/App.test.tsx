import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";

// §6.4.6 unit (G15) — §7.2.1 step 8 (P2.106.8): App is the terminal "hand to UI empty/idle state (§5.2)" step
// of the ordered startup sequence (the src-tauri `main()` spine, P2.106). After the Rust core reveals the
// window (step 6) and feeds launch intake (step 7), control passes to this React shell, which (a) renders the
// §5.2 `Idle` empty state — the `<main>` landmark — and (b) completes the readiness handshake via
// `useLaunchDrain` (C1 `drainPending` → core `mark_ready`, P2.60/P2.61). Mock the §5.8 IPC façade so the mount
// effect stays hermetic under jsdom (no Tauri runtime — the real Channel/invoke throws; the fix from the
// P1.35/ee362ce mount-side-effect note). The drain CALL contract is `lib/ipc/events.test.ts`; the hook's
// gate/once semantics are `useLaunchDrain.test.tsx`; this pins the App-level STEP-8 contract (idle landmark +
// ready handshake fires, gated on listener-registration COMPLETION). [Build-Session-Entscheidung: P2.106.8]
//
// [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] The mount handshake now fires the CONSUMING
// `consumeMountDrain` (routes the `CollectedSet` into the §5.2 machine) rather than the bare
// `drainPendingIntake`; the STEP-8 contract (idle landmark + ready handshake once, gated on registration) is
// unchanged — only the mocked façade name. The `advanceToTargets`/`cancelIntakeCollect` stubs feed the
// statically-imported ConfirmScreen (3) / CollectingScreen (2) router arms P3.55 added — Idle renders the
// DropZone, so they never run here.
const consumeMountDrain = vi.fn<() => Promise<void>>();
const subscribeAppEvents = vi.fn<() => Promise<() => void>>();
const subscribeNativeDragDrop = vi.fn<() => Promise<() => void>>();
vi.mock("./lib/ipc/events", () => ({
  consumeMountDrain: () => consumeMountDrain(),
  subscribeAppEvents: () => subscribeAppEvents(),
  subscribeNativeDragDrop: () => subscribeNativeDragDrop(),
  // The §5.2 Idle screen (the P3.54 DropZone, rendered by App) imports the C2a `pickForIntake` façade; stub it
  // so the Idle render stays hermetic (it fires only on a user action, which this suite does not exercise).
  pickForIntake: () => Promise.resolve(),
  // The P3.55 Collecting (2) / Confirm (3) router arms are statically imported by App; stub their façade calls
  // (unused in the Idle render this suite exercises).
  advanceToTargets: () => Promise.resolve(),
  cancelIntakeCollect: () => Promise.resolve(),
  // The P3.56 Targets (4/5) router arm (TargetsScreen) is statically imported by App; stub its façade calls
  // (fired only on a user action, so unused in these render-only router legs).
  replanOutput: () => Promise.resolve(),
  pickAndSetDestination: () => Promise.resolve(),
  runConversion: () => Promise.resolve(),
}));
// The Confirm (3) router arm renders the ConfirmScreen → BatchSummary, which announces on mount; stub the
// announcer so the router render stays hermetic (its own announce contract is BatchSummary.test.tsx's).
vi.mock("./a11y/announcer", () => ({ announce: () => undefined }));

import { App } from "./App";
import { useAppStore } from "./state/store";
import type { Planned, SingleSet } from "./state/machine";

// Unmount each render (this file did not previously auto-clean; the §5.2 router legs below re-render <App/> per
// machine state, so cleanup keeps their document-scoped role queries from tripping over an accumulated tree).
afterEach(cleanup);

// Drain enough microtask turns that a settled subscribe propagates through useAppEvents' ready deferred into
// useLaunchDrain's gate and the drain invoke lands (three chained `.then` hops; eight turns is safe margin).
async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

describe("App — §7.2.1 step 8 (hand to the §5.2 Idle UI)", () => {
  beforeEach(() => {
    consumeMountDrain.mockReset();
    subscribeAppEvents.mockReset();
    subscribeNativeDragDrop.mockReset();
    consumeMountDrain.mockResolvedValue(undefined);
    subscribeAppEvents.mockResolvedValue(() => {});
    subscribeNativeDragDrop.mockResolvedValue(() => {});
  });

  it("renders the §5.2 Idle `main` landmark and fires all three §5.4/§5.8 mount effects", async () => {
    const { container } = render(<App />);
    // §5.2 Idle empty-state: the `<main>` landmark boots (the step-8 handoff surface; the §5.7 reassurance copy
    // + the per-state screens land P3–P8, so the landmark is the P2 contract).
    expect(container.querySelector("main")).not.toBeNull();
    // §5.4/§5.8: App subscribes the three `app://` listeners (P2.120) + the native file-drop (P2.121) directly
    // on mount, and — once the subscription settles — fires the §7.8.1 readiness drain (P2.60/P2.61), each
    // exactly once.
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
    expect(subscribeNativeDragDrop).toHaveBeenCalledTimes(1);
    await flushMicrotasks();
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] drainPendingIntake→consumeMountDrain rename.
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
  });

  // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] The former invocation-ORDER pin
  // (subscribeAppEvents called before consumeMountDrain in the same synchronous flush) is obsolete: §7.8.1
  // mandates the drain fire "later than listener-registration, so it closes the race" (07-app-shell.md
  // §7.8.1), and mount order alone still let the drain's C1 invoke overtake the three PENDING async `listen`
  // registrations — the core flipped `FrontendReady` while the WebView listeners may not exist, so a second
  // launch in that window was emitted into an unregistered listener and dropped. The correct pin is
  // COMPLETION granularity: no drain while the subscribe is pending; exactly one once it settles.
  it("gates the §7.8.1 drain on listener-registration COMPLETION — no drain while the subscribe is pending", async () => {
    let resolveSubscribe: (cleanup: () => void) => void = () => undefined;
    subscribeAppEvents.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve;
        }),
    );
    render(<App />);
    await flushMicrotasks();
    // The three `listen` registrations are still PENDING: the drain C1 invoke must not have been issued
    // (this is exactly the §7.8.1 unregistered-listener window the gate closes).
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] drainPendingIntake→consumeMountDrain rename.
    expect(consumeMountDrain).not.toHaveBeenCalled();
    resolveSubscribe(() => {});
    await flushMicrotasks();
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
  });

  it("still drains exactly once when the subscribe REJECTS — the buffered set is never stranded", async () => {
    // The drain's buffered set returns via the C1 command RESPONSE, not via an event, so draining after a
    // failed subscribe still loses nothing (§7.8.1) — the reject leg opens the same gate, exactly once.
    subscribeAppEvents.mockRejectedValue(new Error("listen failed"));
    render(<App />);
    await flushMicrotasks();
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] the synchronous invocation-ORDER pin that
    // closed this file is superseded by the two COMPLETION-granularity tests above (order alone never
    // closed the §7.8.1 race — see the tag block above the gating test).
  });
});

// §6.4.6 unit (G15): the §5.2 screen router (P3.55). The Idle → DropZone arm is exercised by the step-8 suite
// above; these legs pin the Collecting (2) + Confirm (3) arms the consumption seam drives into, and that a
// not-yet-built slice state renders the empty `<main>` workspace (never a dead screen). [Build-Session-Entscheidung: P3.55]
describe("App — §5.2 screen router (P3.55)", () => {
  const singleSet: SingleSet = {
    id: "cs1",
    instance: "inst-1",
    format: "csv",
    items: [],
    count: 2,
    skipped: [],
    totalBytes: 10,
    rootsDisplay: ["/drop"],
    encodingHint: null,
    delimiterHint: null,
    notes: [],
  };

  afterEach(() => {
    useAppStore.setState({ machine: { tag: "idle" } });
  });

  it("routes Collecting (2) → the CollectingScreen status region", () => {
    useAppStore.setState({ machine: { tag: "collecting", collectingId: "c1", scanned: null } });
    const { getByRole } = render(<App />);
    expect(getByRole("status")).not.toBeNull();
  });

  it("routes Confirm (3) → the ConfirmScreen", () => {
    useAppStore.setState({ machine: { tag: "confirm", set: singleSet } });
    const { getByRole } = render(<App />);
    expect(getByRole("button", { name: "Continue" })).not.toBeNull();
  });

  it("routes Targets (4/5) → the TargetsScreen (P3.56)", () => {
    const plan: Planned = {
      set: singleSet,
      offer: {
        set: "cs1",
        targets: [
          {
            id: { format: "tsv" },
            label: "TSV",
            lossy: null,
            availability: "available",
            options: [],
          },
        ],
        defaultTarget: { format: "tsv" },
      },
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
    useAppStore.setState({ machine: { tag: "targets", plan } });
    const { getByRole } = render(<App />);
    expect(getByRole("button", { name: "Convert" })).not.toBeNull();
  });

  it("routes RerunPrompt (6) → the RerunScreen modal (P3.57)", () => {
    const plan: Planned = {
      set: singleSet,
      offer: {
        set: "cs1",
        targets: [
          {
            id: { format: "tsv" },
            label: "TSV",
            lossy: null,
            availability: "available",
            options: [],
          },
        ],
        defaultTarget: { format: "tsv" },
      },
      selected: { format: "tsv" },
      options: {},
      destination: "besideSource",
      preview: {
        set: "cs1",
        finalDirDisplay: "/drop",
        diverted: null,
        rerun: { equivalentCount: 2 },
        preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
      },
      persistedFallback: false,
    };
    useAppStore.setState({ machine: { tag: "rerunPrompt", plan, rerun: { equivalentCount: 2 } } });
    const { getByRole } = render(<App />);
    expect(
      getByRole("alertdialog", { name: "Already converted with these settings" }),
    ).not.toBeNull();
    expect(getByRole("button", { name: "Skip" })).not.toBeNull();
  });

  it("renders the empty `<main>` for a not-yet-built slice state (never a dead screen)", () => {
    useAppStore.setState({ machine: { tag: "mixedDropRefusal", found: [] } });
    const { container } = render(<App />);
    const main = container.querySelector("main");
    expect(main).not.toBeNull();
    expect(main?.children.length).toBe(0);
  });
});
