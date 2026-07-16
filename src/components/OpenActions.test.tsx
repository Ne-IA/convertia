import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 OpenActions — the §7.7 open-folder shell-out. Mock the §5.1 events façade (the C9
// `open_path` round-trip); the assertions pin the §5.3/§7.7.1 [DECIDED] contract: the WebView names an
// `OpenTarget` ID, never a path (§7.7.2 / the 2026-07-06 core-owned-paths ruling), and a diverted run gets TWO
// labelled buttons + the connector (a single button would strand a user whose files diverted).
// [Build-Session-Entscheidung: P3.59]
const openResultTarget = vi.fn<(...args: unknown[]) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  openResultTarget: (...args: unknown[]) => openResultTarget(...args),
}));

import { OpenActions } from "./OpenActions";

afterEach(cleanup);
beforeEach(() => {
  openResultTarget.mockReset();
  openResultTarget.mockResolvedValue(undefined);
});

describe("OpenActions — §5.3 / §7.7 (Summary-only)", () => {
  describe("no divert (§1.12 divertRootDisplay === null)", () => {
    it("renders the SINGLE common-root button labelled 'Open folder'", () => {
      const { getByRole, queryByRole } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay={null} />,
      );
      expect(getByRole("button", { name: "Open folder" })).not.toBeNull();
      expect(queryByRole("button", { name: "Open saved-to folder" })).toBeNull();
    });

    it("fires C9 open_path with the CommonRoot OpenTarget id — never a path (§7.7.2)", () => {
      const { getByRole } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay={null} />,
      );
      fireEvent.click(getByRole("button", { name: "Open folder" }));
      expect(openResultTarget).toHaveBeenCalledWith("commonRoot");
    });

    it("renders no split connector line when nothing diverted", () => {
      const { queryByText } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay={null} />,
      );
      expect(queryByText(/Some files were saved to/)).toBeNull();
    });
  });

  describe("split divert (§2.7.3 — divertRootDisplay is present)", () => {
    it("renders BOTH labelled buttons + the connector naming the divert root (§5.3 [DECIDED])", () => {
      const { getByRole, getByText } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />,
      );
      expect(getByRole("button", { name: "Open source folder" })).not.toBeNull();
      expect(getByRole("button", { name: "Open saved-to folder" })).not.toBeNull();
      expect(getByText("Some files were saved to /Downloads")).not.toBeNull();
    });

    it("the source-folder button fires the CommonRoot id", () => {
      const { getByRole } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />,
      );
      fireEvent.click(getByRole("button", { name: "Open source folder" }));
      expect(openResultTarget).toHaveBeenCalledWith("commonRoot");
    });

    it("the saved-to button fires the DivertRoot id (the second §7.7.1 target)", () => {
      const { getByRole } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />,
      );
      fireEvent.click(getByRole("button", { name: "Open saved-to folder" }));
      expect(openResultTarget).toHaveBeenCalledWith("divertRoot");
    });

    it("never renders the un-split 'Open folder' label when the run diverted", () => {
      const { queryByRole } = render(
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />,
      );
      expect(queryByRole("button", { name: "Open folder" })).toBeNull();
    });
  });

  it("passes NO filesystem path to C9 in any rendering (§7.7.2 — the id IS the whole argument)", () => {
    const { getByRole } = render(
      <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />,
    );
    fireEvent.click(getByRole("button", { name: "Open source folder" }));
    fireEvent.click(getByRole("button", { name: "Open saved-to folder" }));
    for (const call of openResultTarget.mock.calls) {
      expect(call).toEqual([expect.stringMatching(/^(commonRoot|divertRoot)$/)]);
    }
  });
});
