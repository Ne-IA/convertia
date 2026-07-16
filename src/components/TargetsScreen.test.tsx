import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 Targets/Destination screen (states 4/5) — the state-4/5 out-transitions (the P3
// screen-box wiring model: a rendered action MUST fire its command). Mock the §5.1 events façade (the C4 re-plan /
// C2b→C5 change / C6 convert) + the announcer; the §5.2 machine is the REAL store so a dispatch (convert → Rerun,
// Back → Confirm) is exercised end-to-end. [Build-Session-Entscheidung: P3.56]
const replanOutput = vi.fn<(...args: unknown[]) => Promise<void>>();
const pickAndSetDestination = vi.fn<(...args: unknown[]) => Promise<void>>();
const runConversion = vi.fn<(...args: unknown[]) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  replanOutput: (...args: unknown[]) => replanOutput(...args),
  pickAndSetDestination: (...args: unknown[]) => pickAndSetDestination(...args),
  runConversion: (...args: unknown[]) => runConversion(...args),
}));

import { TargetsScreen } from "./TargetsScreen";
import { useAppStore } from "../state/store";
import type { Planned, SingleSet } from "../state/machine";

// The frontend tsconfig omits @types/node (frontend code never touches Node globals), so `process` is not a typed
// global here. Vitest runs on Node, so the object exists at runtime; the guard-reset test below reaches its
// `unhandledRejection` bookkeeping through globalThis via this minimal listener-host shape (no @types/node, no `any`) —
// the type-safe alternative to widening the test tsconfig with node types. [Build-Session-Entscheidung: P3.56]
type UnhandledRejectionListener = (...args: unknown[]) => void;
interface RejectionListenerHost {
  listeners(event: "unhandledRejection"): UnhandledRejectionListener[];
  removeAllListeners(event: "unhandledRejection"): void;
  on(event: "unhandledRejection", listener: UnhandledRejectionListener): void;
}
const nodeProcess = (globalThis as unknown as { process: RejectionListenerHost }).process;

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

const plan: Planned = {
  set: singleSet,
  offer: {
    set: "cs1",
    targets: [
      { id: { format: "tsv" }, label: "TSV", lossy: null, availability: "available", options: [] },
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

// Seat the store's machine to match the rendered prop, so a dispatched Msg (which the machine reduces over the
// STORE state) transitions correctly, then render TargetsScreen from the same plan.
function renderTargets(held: Planned = plan) {
  useAppStore.setState({ machine: { tag: "targets", plan: held } });
  return render(<TargetsScreen plan={held} />);
}

afterEach(cleanup);
beforeEach(() => {
  replanOutput.mockReset();
  replanOutput.mockResolvedValue(undefined);
  pickAndSetDestination.mockReset();
  pickAndSetDestination.mockResolvedValue(undefined);
  runConversion.mockReset();
  runConversion.mockResolvedValue(undefined);
});

describe("TargetsScreen — §5.2 Targets/Destination (states 4/5)", () => {
  it("composes the FormatPicker (tiles) + DestinationBar (will-save-to + Change + Convert)", () => {
    const { getByRole, getByText } = renderTargets();
    expect(getByRole("button", { name: "TSV" })).not.toBeNull();
    expect(getByText("Will save to /drop")).not.toBeNull();
    expect(getByRole("button", { name: "Convert" })).not.toBeNull();
  });

  it("a tile select dispatches selectTarget + fires the C4 re-plan (replanOutput)", () => {
    const { getByRole } = renderTargets();
    fireEvent.click(getByRole("button", { name: "TSV" }));
    expect(replanOutput).toHaveBeenCalledWith("cs1", { format: "tsv" }, {}, "besideSource");
  });

  it("Change destination fires the C2b→C5 flow (pickAndSetDestination) for the held (set, target, options)", () => {
    const { getByRole } = renderTargets();
    fireEvent.click(getByRole("button", { name: "Change destination" }));
    expect(pickAndSetDestination).toHaveBeenCalledWith("cs1", { format: "tsv" }, {});
  });

  it("Convert with no rerun verdict fires C6 (runConversion, decision skip) → Converting", () => {
    const { getByRole } = renderTargets();
    fireEvent.click(getByRole("button", { name: "Convert" }));
    expect(runConversion).toHaveBeenCalledWith(
      "cs1",
      { format: "tsv" },
      {},
      "besideSource",
      "skip",
    );
  });

  it("Convert with a §2.5 rerun verdict shows the RerunPrompt (dispatch convert), firing NO C6", () => {
    const withRerun: Planned = {
      ...plan,
      preview: { ...plan.preview, rerun: { equivalentCount: 2 } },
    };
    const { getByRole } = renderTargets(withRerun);
    fireEvent.click(getByRole("button", { name: "Convert" }));
    expect(useAppStore.getState().machine.tag).toBe("rerunPrompt");
    expect(runConversion).not.toHaveBeenCalled();
  });

  it("guards a double-convert — a rapid double Convert fires C6 once", () => {
    const { getByRole } = renderTargets();
    const convert = getByRole("button", { name: "Convert" });
    fireEvent.click(convert);
    fireEvent.click(convert);
    expect(runConversion).toHaveBeenCalledTimes(1);
  });

  it("resets the convert guard on a C6 rejection so the user can retry (§5.8 fail-clear)", async () => {
    // The component re-throws a C6 rejection to the §7.5.1 global bridge (the ConfirmScreen re-throw convention) —
    // a DELIBERATE unhandled rejection here (installFrontendErrorLog's listener is ADDITIVE, no preventDefault).
    // Vitest tracks unhandled rejections at the Node process level (a jsdom `window` preventDefault does NOT
    // suppress it), so swap Vitest's `unhandledRejection` listeners for a swallow across the intentional throw; the
    // throw's real observability is the §7.5.1 bridge's concern, not this guard-reset test's.
    const saved = nodeProcess.listeners("unhandledRejection");
    nodeProcess.removeAllListeners("unhandledRejection");
    nodeProcess.on("unhandledRejection", () => undefined);
    try {
      runConversion.mockRejectedValueOnce(new Error("ipc drop")); // first C6 rejects; the beforeEach default resolves
      const { getByRole } = renderTargets();
      const convert = getByRole("button", { name: "Convert" });
      fireEvent.click(convert); // fires runConversion #1 → rejects → `.catch` resets the guard + re-throws
      await new Promise((resolve) => setTimeout(resolve, 0)); // let the reject + the `.catch` reset settle
      fireEvent.click(convert); // the guard was reset → fires runConversion #2 (a clean retry)
      expect(runConversion).toHaveBeenCalledTimes(2);
    } finally {
      nodeProcess.removeAllListeners("unhandledRejection");
      for (const listener of saved) {
        nodeProcess.on("unhandledRejection", listener);
      }
    }
  });

  it("Back dispatches `back` → Confirm, preserving the frozen set (§5.2 row 4)", () => {
    const { getByRole } = renderTargets();
    fireEvent.click(getByRole("button", { name: "Back" }));
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("confirm");
    if (machine.tag === "confirm") {
      expect(machine.set).toBe(singleSet);
    }
    expect(runConversion).not.toHaveBeenCalled();
  });
});
