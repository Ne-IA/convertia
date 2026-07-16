import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 ProgressList — the per-item rows + the aggregate bar over the store's reduced
// §5.8 `ItemRow`s. Purely presentational (props), so no store/events mock. Pins the aggregate label + bar, the
// per-item source display + status label + running progress bar, the terminal transitions (Succeeded/Failed/
// Cancelled), the verbatim §2.8 Failed reason, and the numeric row order. [Build-Session-Entscheidung: P3.58]
import { ProgressList } from "./ProgressList";
import type { ItemRow } from "../state/store";

const running = (sourceDisplay: string, fraction: number | null): ItemRow => ({
  sourceDisplay,
  status: "running",
  fraction,
  reason: null,
});

afterEach(cleanup);

describe("ProgressList — §5.3 / §1.11 live progress", () => {
  it("renders the aggregate label + a progressbar when batchProgress is present", () => {
    const { getByText, getByRole } = render(
      <ProgressList rows={{ 0: running("/a.csv", 0.5) }} batchProgress={{ done: 1, total: 2 }} />,
    );
    expect(getByText("1 of 2 files done")).not.toBeNull();
    // The aggregate <progress> is the one named by the aggregate line.
    expect(getByRole("progressbar", { name: "1 of 2 files done" })).not.toBeNull();
  });

  it("uses the singular aggregate template for a single-file batch", () => {
    const { getByText } = render(
      <ProgressList rows={{ 0: running("/a.csv", 1) }} batchProgress={{ done: 0, total: 1 }} />,
    );
    expect(getByText("0 of 1 file done")).not.toBeNull();
  });

  it("omits the aggregate when batchProgress is null (before the first BatchProgress tick)", () => {
    const { queryByText } = render(
      <ProgressList rows={{ 0: running("/a.csv", 0.3) }} batchProgress={null} />,
    );
    expect(queryByText(/files? done/)).toBeNull();
  });

  it("renders a running row: source display + 'Converting…' + a determinate progressbar named by the row", () => {
    const { getByText, getByRole } = render(
      <ProgressList rows={{ 0: running("/a.csv", 0.5) }} batchProgress={null} />,
    );
    expect(getByText("/a.csv")).not.toBeNull();
    expect(getByText("Converting…")).not.toBeNull();
    const bar = getByRole("progressbar", { name: "/a.csv" });
    expect(bar.getAttribute("value")).toBe("0.5");
  });

  it("renders a succeeded row as terminal 'Done' with no progressbar", () => {
    const { getByText, queryByRole } = render(
      <ProgressList
        rows={{ 0: { sourceDisplay: "/a.csv", status: "succeeded", fraction: 1, reason: null } }}
        batchProgress={null}
      />,
    );
    expect(getByText("Done")).not.toBeNull();
    // A terminal row shows its status label, not a live bar.
    expect(queryByRole("progressbar")).toBeNull();
  });

  it("renders a failed row as terminal 'Failed' with the verbatim §2.8 reason", () => {
    const { getByText } = render(
      <ProgressList
        rows={{
          0: {
            sourceDisplay: "/a.csv",
            status: "failed",
            fraction: 0.4,
            reason: "Couldn't read this file.",
          },
        }}
        batchProgress={null}
      />,
    );
    expect(getByText("Failed")).not.toBeNull();
    expect(getByText("Couldn't read this file.")).not.toBeNull();
  });

  it("renders a cancelled row as terminal 'Cancelled'", () => {
    const { getByText } = render(
      <ProgressList
        rows={{ 0: { sourceDisplay: "/a.csv", status: "cancelled", fraction: 0.3, reason: null } }}
        batchProgress={null}
      />,
    );
    expect(getByText("Cancelled")).not.toBeNull();
  });

  it("renders a skipped row as terminal 'Skipped' (the box's fourth terminal state)", () => {
    const { getByText } = render(
      <ProgressList
        rows={{ 0: { sourceDisplay: "/a.csv", status: "skipped", fraction: 0.1, reason: null } }}
        batchProgress={null}
      />,
    );
    expect(getByText("Skipped")).not.toBeNull();
  });

  it("orders rows by numeric itemId, not object-key insertion order", () => {
    const { container } = render(
      <ProgressList
        rows={{ 10: running("/ten.csv", 0.1), 2: running("/two.csv", 0.2) }}
        batchProgress={null}
      />,
    );
    const names = Array.from(container.querySelectorAll("li > span:first-child")).map(
      (el) => el.textContent,
    );
    expect(names).toEqual(["/two.csv", "/ten.csv"]);
  });
});
