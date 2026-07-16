import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 RerunPrompt screen (state 6) — the state-6 out-transitions (the P3 screen-box
// wiring model: a rendered action MUST fire its command). Mock the §5.1 events façade (the C6 convert +, for the
// inert TargetsScreen backdrop, its C4 re-plan / C2b→C5 change); the §5.2 machine is the REAL store so a dispatch
// (Cancel → rerunCancel → Targets) is exercised end-to-end. [Build-Session-Entscheidung: P3.57]
const replanOutput = vi.fn<(...args: unknown[]) => Promise<void>>();
const pickAndSetDestination = vi.fn<(...args: unknown[]) => Promise<void>>();
const runConversion = vi.fn<(...args: unknown[]) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  replanOutput: (...args: unknown[]) => replanOutput(...args),
  pickAndSetDestination: (...args: unknown[]) => pickAndSetDestination(...args),
  runConversion: (...args: unknown[]) => runConversion(...args),
}));

import { RerunScreen } from "./RerunScreen";
import { useAppStore } from "../state/store";
import type { Planned, SingleSet } from "../state/machine";

// The frontend tsconfig omits @types/node (frontend code never touches Node globals), so `process` is not a typed
// global here. Vitest runs on Node, so the object exists at runtime; the guard-reset test below reaches its
// `unhandledRejection` bookkeeping through globalThis via this minimal listener-host shape (no @types/node, no `any`) —
// the type-safe alternative to widening the test tsconfig with node types (the TargetsScreen.test precedent).
// [Build-Session-Entscheidung: P3.57]
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

// A held plan carrying a §2.5 rerun verdict (the machine only enters state 6 when `preview.rerun !== null`).
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
    rerun: { equivalentCount: 2 },
    preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
  },
  persistedFallback: false,
};

// Seat the store's machine to state 6 (so a dispatched Msg reduces over the STORE state), then render RerunScreen.
function renderRerun(held: Planned = plan) {
  useAppStore.setState({
    machine: { tag: "rerunPrompt", plan: held, rerun: { equivalentCount: 2 } },
  });
  return render(<RerunScreen plan={held} />);
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

describe("RerunScreen — §5.2 RerunPrompt (state 6)", () => {
  it("renders the RerunPrompt modal over the inert Targets/Destination backdrop (§5.3)", () => {
    const { getByRole, container } = renderRerun();
    // The modal's controls.
    expect(getByRole("button", { name: "Skip" })).not.toBeNull();
    // The backdrop: the TargetsScreen is rendered but made `inert` (out of tab order, non-interactive).
    const backdrop = container.querySelector("[inert]");
    expect(backdrop).not.toBeNull();
    expect(backdrop?.querySelector("button")).not.toBeNull();
  });

  it("lands default focus on Skip in the COMPOSED screen, not the inert backdrop's tile (§5.6/§5.10)", () => {
    // Hardens the composition: the modal's default-focus effect must win over the inert backdrop TargetsScreen's
    // FormatPicker mount-focus (a JSX-sibling-order dependency — RerunPrompt is rendered after the inert div, so
    // its focus effect commits last). Guards against a silent regression if that order is ever reversed.
    const { getByRole } = renderRerun();
    expect(document.activeElement).toBe(getByRole("button", { name: "Skip" }));
  });

  it("Skip fires C6 (runConversion, decision skip) for the held (set, target, options, destination)", () => {
    const { getByRole } = renderRerun();
    fireEvent.click(getByRole("button", { name: "Skip" }));
    expect(runConversion).toHaveBeenCalledWith(
      "cs1",
      { format: "tsv" },
      {},
      "besideSource",
      "skip",
    );
  });

  it("Make a fresh copy fires C6 (runConversion, decision freshCopy)", () => {
    const { getByRole } = renderRerun();
    fireEvent.click(getByRole("button", { name: "Make a fresh copy" }));
    expect(runConversion).toHaveBeenCalledWith(
      "cs1",
      { format: "tsv" },
      {},
      "besideSource",
      "freshCopy",
    );
  });

  it("Cancel dispatches `rerunCancel` → Targets, preserving the held plan (§5.2 row 6), firing NO C6", () => {
    const { getByRole } = renderRerun();
    fireEvent.click(getByRole("button", { name: "Cancel" }));
    const machine = useAppStore.getState().machine;
    expect(machine.tag).toBe("targets");
    if (machine.tag === "targets") {
      expect(machine.plan).toBe(plan);
    }
    expect(runConversion).not.toHaveBeenCalled();
  });

  it("Cancel is INERT once a decision has fired C6 — no teleport-to-Converting race (§5.2 row 6)", () => {
    // The double-run race the review caught: Skip fires C6 (guard set) → a Cancel before `runStarted` arrives
    // would return to Targets and let the pending `runStarted` teleport into Converting. The commit-final Cancel
    // guard makes Cancel inert while a decision is in flight, so the machine STAYS in `rerunPrompt` (where the
    // pending `runStarted` resolves correctly via `fromRerunPrompt`), not `targets`.
    const { getByRole } = renderRerun();
    fireEvent.click(getByRole("button", { name: "Skip" })); // commits the decision (C6 in flight)
    fireEvent.click(getByRole("button", { name: "Cancel" })); // must be inert now
    expect(useAppStore.getState().machine.tag).toBe("rerunPrompt");
    expect(runConversion).toHaveBeenCalledTimes(1); // only the Skip; Cancel fired nothing
  });

  it("guards a double-convert across the two decision buttons — Skip then Make-a-fresh-copy fires C6 once", () => {
    const { getByRole } = renderRerun();
    fireEvent.click(getByRole("button", { name: "Skip" }));
    fireEvent.click(getByRole("button", { name: "Make a fresh copy" }));
    expect(runConversion).toHaveBeenCalledTimes(1);
  });

  it("resets the convert guard on a C6 rejection so the user can retry (§5.8 fail-clear)", async () => {
    // RerunScreen re-throws a C6 rejection to the §7.5.1 global bridge (the TargetsScreen convert precedent) — a
    // DELIBERATE unhandled rejection. Vitest tracks unhandled rejections at the Node process level, so swap its
    // `unhandledRejection` listeners for a swallow across the intentional throw (the TargetsScreen.test pattern).
    const saved = nodeProcess.listeners("unhandledRejection");
    nodeProcess.removeAllListeners("unhandledRejection");
    nodeProcess.on("unhandledRejection", () => undefined);
    try {
      runConversion.mockRejectedValueOnce(new Error("ipc drop")); // first C6 rejects; the beforeEach default resolves
      const { getByRole } = renderRerun();
      const skip = getByRole("button", { name: "Skip" });
      fireEvent.click(skip); // fires runConversion #1 → rejects → `.catch` resets the guard + re-throws
      await new Promise((resolve) => setTimeout(resolve, 0)); // let the reject + the `.catch` reset settle
      fireEvent.click(skip); // the guard was reset → fires runConversion #2 (a clean retry)
      expect(runConversion).toHaveBeenCalledTimes(2);
    } finally {
      nodeProcess.removeAllListeners("unhandledRejection");
      for (const listener of saved) {
        nodeProcess.on("unhandledRejection", listener);
      }
    }
  });
});
