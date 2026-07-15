import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 BatchSummary — the §5.2 confirm-gate count line + the §1.4 passive skip tally +
// the §5.6 assertive announcement. Mock the §5.6 announcer so the assertive announce is observable without a
// real live region. [Build-Session-Entscheidung: P3.55]
const announce = vi.fn<(message: string, priority: string) => void>();
vi.mock("../a11y/announcer", () => ({
  announce: (message: string, priority: string) => announce(message, priority),
}));

import { BatchSummary } from "./BatchSummary";

afterEach(cleanup);
beforeEach(() => {
  announce.mockReset();
});

describe("BatchSummary — §5.2 confirm-gate summary", () => {
  it("renders the count line 'N FORMAT files' (the §1.4 detected-format + count)", () => {
    const { getByText, queryByText } = render(
      <BatchSummary count={48} format="csv" skippedCount={0} />,
    );
    expect(getByText("48 CSV files").tagName).toBe("H2");
    // §1.4: no tally when nothing was skipped (the clean-drop case).
    expect(queryByText(/weren't recognized/)).toBeNull();
  });

  it("renders the passive skip tally when >=1 item was skipped (§1.4 never silent)", () => {
    const { getByText } = render(<BatchSummary count={48} format="csv" skippedCount={3} />);
    expect(getByText("3 files weren't recognized and will be skipped")).not.toBeNull();
  });

  it("uses the singular count + tally forms for 1 (§5.6 singular/plural)", () => {
    const { getByText } = render(<BatchSummary count={1} format="tsv" skippedCount={1} />);
    expect(getByText("1 TSV file")).not.toBeNull();
    expect(getByText("1 file wasn't recognized and will be skipped")).not.toBeNull();
  });

  it("announces the combined summary + tally ASSERTIVELY on entry (§5.6/§5.6.1, one source)", () => {
    render(<BatchSummary count={48} format="csv" skippedCount={3} />);
    expect(announce).toHaveBeenCalledWith(
      "48 CSV files — 3 files weren't recognized and will be skipped",
      "assertive",
    );
  });

  it("announces the bare count assertively when nothing was skipped", () => {
    render(<BatchSummary count={2} format="tsv" skippedCount={0} />);
    expect(announce).toHaveBeenCalledWith("2 TSV files", "assertive");
  });
});
