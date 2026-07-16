import { describe, it, expect, afterEach, beforeEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 UnsupportedNotice — the state-10 intake-refusal notice. Pins all FOUR §5.3
// variants (incl. the DERIVED `Unreadable`, which has no wire arm of its own), the §02-supplied payloads that
// must ride through VERBATIM (`detected`, the §1.2 `Uncertain.note`), the §5.2 row-10 Empty skip tally, the
// §5.3:307 focus-on-entry, and the Dismiss/Esc → Idle exit (the P3 screen-box wiring model).
// [Build-Session-Entscheidung: P3.60]
import { UnsupportedNotice, resolveVariant } from "./UnsupportedNotice";
import { useAppStore } from "../state/store";
import type { SkippedItem } from "../lib/ipc/commands";

function skipped(item: number, reason: SkippedItem["reason"]): SkippedItem {
  return { item, sourceDisplay: `f${item}.bin`, detectedDisplay: null, reason };
}

beforeEach(() => {
  useAppStore.setState({
    machine: { tag: "unsupported", reason: { kind: "unsupported", detected: "PDF" } },
  });
});
afterEach(() => {
  cleanup();
  useAppStore.setState({ machine: { tag: "idle" } });
});

describe("UnsupportedNotice — §5.2 Unsupported (state 10)", () => {
  it("Unsupported: renders the detected type IN the heading (§5.2 row 10 'detected: X')", () => {
    const { getByRole } = render(
      <UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />,
    );
    expect(
      getByRole("heading", { name: "Can't convert this type — detected: PDF" }),
    ).not.toBeNull();
  });

  it("Uncertain: renders the §1.2 note VERBATIM as a calm secondary line — the payload is never dropped", () => {
    // §5.2 row 10 / §5.3:307: `Uncertain.note` is core-produced text; the UI renders it, never paraphrases it.
    const note = "The header says PNG but the tail looks like a ZIP archive.";
    const { getByRole, getByText } = render(
      <UnsupportedNotice reason={{ kind: "uncertain", note }} />,
    );
    expect(getByRole("heading", { name: "Couldn't tell what this file is" })).not.toBeNull();
    expect(getByText(note)).not.toBeNull();
  });

  it("Empty: renders the 'nothing here' copy + the §5.2 row-10 per-reason skip tally", () => {
    const { getByRole, getByText } = render(
      <UnsupportedNotice
        reason={{
          kind: "empty",
          skipped: [skipped(0, "unreadable"), skipped(1, "unsupportedType"), skipped(2, "empty")],
        }}
      />,
    );
    expect(getByRole("heading", { name: "Nothing here I can convert" })).not.toBeNull();
    expect(
      getByText("3 files, none convertible (1 unreadable, 1 unsupported, 1 empty)"),
    ).not.toBeNull();
  });

  it("Unreadable (DERIVED): an all-unreadable Empty renders its OWN copy, not the generic Empty line", () => {
    // §5.2 row 10's "or every collected item was unreadable/gone" entry condition — it has no wire arm, so it
    // is derived from the skip-reason set (the module's [Derived-Assumption: P3.60]).
    const { getByRole } = render(
      <UnsupportedNotice
        reason={{ kind: "empty", skipped: [skipped(0, "unreadable"), skipped(1, "unreadable")] }}
      />,
    );
    expect(getByRole("heading", { name: "Couldn't read these files" })).not.toBeNull();
  });

  it("a MIXED skip set is NOT Unreadable — one non-unreadable item keeps the generic Empty copy", () => {
    const { getByRole } = render(
      <UnsupportedNotice
        reason={{ kind: "empty", skipped: [skipped(0, "unreadable"), skipped(1, "empty")] }}
      />,
    );
    expect(getByRole("heading", { name: "Nothing here I can convert" })).not.toBeNull();
  });

  it("the all-hidden drop (Empty { skipped: [] }) renders the plain copy and NO tally (§5.2 row 10)", () => {
    const { getByRole, queryByText } = render(
      <UnsupportedNotice reason={{ kind: "empty", skipped: [] }} />,
    );
    // The vacuous "every item is unreadable" arm must NOT fire on an empty set — it is the plain Empty case.
    expect(getByRole("heading", { name: "Nothing here I can convert" })).not.toBeNull();
    expect(queryByText(/none convertible/)).toBeNull();
  });

  it("resolveVariant maps the three machine arms onto the four §5.3 render variants", () => {
    expect(resolveVariant({ kind: "unsupported", detected: "PDF" }).heading).toBe(
      "Can't convert this type — detected: PDF",
    );
    expect(resolveVariant({ kind: "uncertain", note: "n" }).note).toBe("n");
    expect(resolveVariant({ kind: "empty", skipped: [skipped(0, "unreadable")] }).heading).toBe(
      "Couldn't read these files",
    );
    expect(resolveVariant({ kind: "empty", skipped: [] }).tally).toBeNull();
  });

  it("announces its heading assertively and focuses the DISMISS button, never the heading (§5.3:307)", () => {
    const { getByRole } = render(
      <UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />,
    );
    const heading = getByRole("heading", { name: /Can't convert this type/ });
    expect(heading.getAttribute("aria-live")).toBe("assertive");
    // Focus on Dismiss so Enter activates it (§5.10:1241) — a tabindex=-1 heading would make Enter a no-op.
    expect(document.activeElement).toBe(getByRole("button", { name: "Dismiss" }));
  });

  it("Dismiss dispatches dismiss → the machine returns to Idle (§5.2 row 10)", () => {
    const { getByRole } = render(
      <UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />,
    );
    fireEvent.click(getByRole("button", { name: "Dismiss" }));
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("Esc dismisses → Idle (§5.10:1231)", () => {
    render(<UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("is NOT a modal — a full-screen state, so no alertdialog/focus trap (§5.7:840)", () => {
    const { queryByRole } = render(
      <UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />,
    );
    expect(queryByRole("alertdialog")).toBeNull();
    expect(queryByRole("dialog")).toBeNull();
  });
});
