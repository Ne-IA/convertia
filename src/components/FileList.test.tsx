import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 FileList — the "Show N files" disclosure + the eligible/skipped rows. The §1.10
// windowing MATH is unit-tested in useVirtualWindow.test.ts; here jsdom renders a small list (all rows in the
// window) so the disclosure toggle + the eligible-vs-skipped rendering are pinned. [Build-Session-Entscheidung: P3.55]
import { FileList, type FileListItem } from "./FileList";
import type { SkippedItem } from "../lib/ipc/commands";
import { ui } from "../strings/ui";

afterEach(cleanup);

const items: FileListItem[] = [
  { item: 1, displayName: "a.csv", relPathDisplay: null },
  { item: 2, displayName: "b.csv", relPathDisplay: "sub/b.csv" },
];
const skipped: SkippedItem[] = [
  { item: 3, sourceDisplay: "notes.pdf", detectedDisplay: "PDF", reason: "unsupportedType" },
];

describe("FileList — §5.3 confirm-gate per-item disclosure", () => {
  it("is collapsed by default: 'Show N files' (N = eligible + skipped), rows not rendered", () => {
    const { getByRole, queryByText } = render(<FileList items={items} skipped={skipped} />);
    const toggle = getByRole("button", { name: "Show 3 files" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    // Collapsed: no per-item rows in the DOM yet.
    expect(queryByText("a.csv")).toBeNull();
    expect(queryByText("notes.pdf")).toBeNull();
  });

  it("expands to reveal every collected row (eligible + skipped), toggling the disclosure label", () => {
    const { getByRole, getByText } = render(<FileList items={items} skipped={skipped} />);
    fireEvent.click(getByRole("button", { name: "Show 3 files" }));
    const toggle = getByRole("button", { name: "Hide 3 files" });
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    // Eligible rows: display name + (folder) rel-path preview.
    expect(getByText("a.csv")).not.toBeNull();
    expect(getByText("b.csv")).not.toBeNull();
    expect(getByText("sub/b.csv")).not.toBeNull();
    // Skipped row: source display + the §5.3 reason line (textual, §5.6 not colour-alone).
    expect(getByText("notes.pdf")).not.toBeNull();
    expect(getByText("Skipped — Unsupported type — detected: PDF")).not.toBeNull();
  });

  it("marks the skipped row distinctly from eligible rows (data-skipped)", () => {
    const { getByRole, getByText } = render(<FileList items={items} skipped={skipped} />);
    fireEvent.click(getByRole("button", { name: "Show 3 files" }));
    const skippedRow = getByText("notes.pdf").closest("[data-skipped]");
    expect(skippedRow?.getAttribute("data-skipped")).toBe("true");
    // An eligible row carries no skipped marker.
    expect(getByText("a.csv").closest("[data-skipped]")).toBeNull();
  });

  it("collapses again on a second toggle (rows removed, aria-expanded false)", () => {
    const { getByRole, queryByText } = render(<FileList items={items} skipped={skipped} />);
    fireEvent.click(getByRole("button", { name: "Show 3 files" }));
    fireEvent.click(getByRole("button", { name: "Hide 3 files" }));
    expect(getByRole("button", { name: "Show 3 files" }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(queryByText("a.csv")).toBeNull();
  });

  it("exposes the expanded list as a keyboard-focusable, named scroll region (§5.6 keyboard scroll)", () => {
    const { getByRole } = render(<FileList items={items} skipped={skipped} />);
    fireEvent.click(getByRole("button", { name: "Show 3 files" }));
    // The rows are non-focusable (read-only), so the scroll REGION must be focusable + named so a keyboard/AT
    // user can Tab to it and arrow/Page-scroll past the first window.
    const region = getByRole("group", { name: ui.filelist_region_label });
    expect(region.getAttribute("tabindex")).toBe("0");
  });

  it("renders skipped-reason variants without a detected name (§0.6 detectedDisplay null)", () => {
    const noDetect: SkippedItem[] = [
      { item: 9, sourceDisplay: "empty.dat", detectedDisplay: null, reason: "unreadable" },
    ];
    const { getByRole, getByText } = render(<FileList items={[]} skipped={noDetect} />);
    fireEvent.click(getByRole("button", { name: "Show 1 file" }));
    expect(getByText("Skipped — Couldn't read this file")).not.toBeNull();
  });
});
