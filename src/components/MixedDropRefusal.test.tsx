import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 MixedDropRefusal — the §1.3 hard pre-flight refusal (state 9). Pins the §5.2
// row-9 contract: the formats-found tally, the ACTIVE re-drop DropZone as the primary action (§5.3 [DECIDED]),
// the Dismiss/Esc → Idle exit (the P3 screen-box wiring model: a rendered action MUST fire its command), the
// §5.3:306 focus-on-entry, and the §5.10:1211 global-chord gate. Mock the §5.1 events façade — the composed
// DropZone fires C2a through it. [Build-Session-Entscheidung: P3.60]
const pickForIntake = vi.fn<(kind: string) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({ pickForIntake: (kind: string) => pickForIntake(kind) }));

import { MixedDropRefusal } from "./MixedDropRefusal";
import { useAppStore } from "../state/store";
import type { MixedFound } from "../state/machine";

const found: MixedFound = [
  ["jpg", 30],
  ["png", 12],
  ["pdf", 3],
];

beforeEach(() => {
  pickForIntake.mockReset();
  pickForIntake.mockResolvedValue(undefined);
  useAppStore.setState({ machine: { tag: "mixedDropRefusal", found } });
});
afterEach(() => {
  cleanup();
  useAppStore.setState({ machine: { tag: "idle" } });
});

describe("MixedDropRefusal — §5.2 MixedDropRefusal (state 9)", () => {
  it("lists every format found with its count (§5.2 row 9), in the wire's order", () => {
    const { getByText } = render(<MixedDropRefusal found={found} />);
    expect(getByText("Found 30 JPG, 12 PNG, 3 PDF")).not.toBeNull();
  });

  it("announces its heading via an assertive live region, and does NOT focus it (§5.3:306)", () => {
    const { getByRole } = render(<MixedDropRefusal found={found} />);
    const heading = getByRole("heading", { name: "More than one kind of file" });
    expect(heading.getAttribute("aria-live")).toBe("assertive");
    expect(document.activeElement).not.toBe(heading);
  });

  it("focuses the re-drop DropZone on entry — the PRIMARY action (§5.3:306 [DECIDED])", () => {
    const { getByRole } = render(<MixedDropRefusal found={found} />);
    expect(document.activeElement).toBe(getByRole("button", { name: /Drop files here/ }));
  });

  it("renders an ACTIVE (not disabled) re-drop DropZone that fires C2a — state 9 is pre-flight (§5.3)", () => {
    const { getByRole } = render(<MixedDropRefusal found={found} />);
    const surface = getByRole("button", { name: /Drop files here/ });
    expect((surface as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(surface);
    expect(pickForIntake).toHaveBeenCalledWith("files");
  });

  it("does NOT bind the §5.10:1211 global chords — Ctrl+O is Idle-only; state 9 re-drops via the focused surface", () => {
    render(<MixedDropRefusal found={found} />);
    fireEvent.keyDown(document, { key: "o", ctrlKey: true });
    fireEvent.keyDown(document, { key: "o", ctrlKey: true, shiftKey: true });
    expect(pickForIntake).not.toHaveBeenCalled();
  });

  it("Dismiss dispatches dismiss → the machine returns to Idle (§5.2 row 9)", () => {
    const { getByRole } = render(<MixedDropRefusal found={found} />);
    fireEvent.click(getByRole("button", { name: "Dismiss" }));
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("Esc is the secondary Dismiss → Idle (§5.10:1232)", () => {
    render(<MixedDropRefusal found={found} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("offers NO subset-convert affordance — the refusal is whole-batch (§5.2 row 9, parked for v1)", () => {
    const { queryByRole } = render(<MixedDropRefusal found={found} />);
    // The screen's only actions are the re-drop surface, its choose-folder sibling, and Dismiss — no
    // "convert the JPGs anyway" per-format button exists.
    expect(queryByRole("button", { name: /convert/i })).toBeNull();
  });

  it("is NOT a modal — a full-screen state, so no alertdialog/focus trap (§5.7:840)", () => {
    const { queryByRole } = render(<MixedDropRefusal found={found} />);
    expect(queryByRole("alertdialog")).toBeNull();
    expect(queryByRole("dialog")).toBeNull();
  });
});
