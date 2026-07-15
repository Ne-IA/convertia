import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 FileList disclosure — collapsed AND expanded (the virtualised list
// mounted). Renders INTO a `<main>` like the other per-screen a11y legs. [Build-Session-Entscheidung: P3.55]
import { FileList, type FileListItem } from "./FileList";
import type { SkippedItem } from "../lib/ipc/commands";

afterEach(cleanup);

const items: FileListItem[] = [{ item: 1, displayName: "a.csv", relPathDisplay: null }];
const skipped: SkippedItem[] = [
  { item: 2, sourceDisplay: "notes.pdf", detectedDisplay: "PDF", reason: "unsupportedType" },
];

describe("FileList — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations while collapsed (the disclosure button, aria-expanded)", async () => {
    const { container } = render(
      <main>
        <FileList items={items} skipped={skipped} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders with no axe violations while expanded (the virtualised row list mounted)", async () => {
    const { container, getByRole } = render(
      <main>
        <FileList items={items} skipped={skipped} />
      </main>,
    );
    fireEvent.click(getByRole("button", { name: "Show 2 files" }));
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
