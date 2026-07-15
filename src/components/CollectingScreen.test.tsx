import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 Collecting screen — the role="status" scan indicator + the §5.10 cancel-collect
// (Esc / button → C13). Mock the §5.1 events façade so the C13 fire is observable with no Tauri runtime.
// [Build-Session-Entscheidung: P3.55]
const cancelIntakeCollect = vi.fn<(collectingId: string) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  cancelIntakeCollect: (collectingId: string) => cancelIntakeCollect(collectingId),
}));

import { CollectingScreen } from "./CollectingScreen";
import { ui } from "../strings/ui";

afterEach(cleanup);
beforeEach(() => {
  cancelIntakeCollect.mockReset();
  cancelIntakeCollect.mockResolvedValue(undefined);
});

describe("CollectingScreen — §5.2 Collecting status + cancel-collect", () => {
  it("renders the throttled scan count in a role=status region (§5.6.1 state-2 landing)", () => {
    const { getByRole } = render(<CollectingScreen collectingId="c1" scanned={42} />);
    expect(getByRole("status").textContent).toBe("Scanning… 42 files so far");
  });

  it("renders the indeterminate fallback until a count arrives (scanned === null)", () => {
    const { getByRole } = render(<CollectingScreen collectingId="c1" scanned={null} />);
    expect(getByRole("status").textContent).toBe("Looking at your files…");
  });

  it("fires C13 cancel_ingest for this walk on the Cancel button (§5.10)", () => {
    const { getByRole } = render(<CollectingScreen collectingId="c1" scanned={null} />);
    fireEvent.click(getByRole("button", { name: ui.collecting_cancel }));
    expect(cancelIntakeCollect).toHaveBeenCalledWith("c1");
  });

  it("fires C13 cancel_ingest on Esc (§5.10 cancel-collect)", () => {
    render(<CollectingScreen collectingId="c1" scanned={7} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelIntakeCollect).toHaveBeenCalledWith("c1");
  });

  it("unbinds the Esc listener on unmount (no cancel after leaving Collecting)", () => {
    const { unmount } = render(<CollectingScreen collectingId="c1" scanned={null} />);
    unmount();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelIntakeCollect).not.toHaveBeenCalled();
  });
});
