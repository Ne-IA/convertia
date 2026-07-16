import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 FormatPicker (state 4) — the target tiles, the pre-highlighted default (aria-pressed
// + visual), the onSelect fire, and focus-on-entry (§5.6.1). Pure presentational (props only, no store/façade), so
// no mocks. [Build-Session-Entscheidung: P3.56]
import { FormatPicker, targetKey } from "./FormatPicker";
import type { Target } from "../lib/ipc/commands";

const tsv: Target = {
  id: { format: "tsv" },
  label: "TSV",
  lossy: null,
  availability: "available",
  options: [],
};
const csv: Target = {
  id: { format: "csv" },
  label: "CSV",
  lossy: null,
  availability: "available",
  options: [],
};

afterEach(cleanup);

describe("FormatPicker — §5.3 target tiles (state 4)", () => {
  it("renders one tile per offered target with the backend-supplied label (§1.5, from C3)", () => {
    const { getByRole } = render(
      <FormatPicker targets={[tsv, csv]} selected={{ format: "tsv" }} onSelect={() => undefined} />,
    );
    expect(getByRole("button", { name: "TSV" })).not.toBeNull();
    expect(getByRole("button", { name: "CSV" })).not.toBeNull();
  });

  it("marks ONLY the selected tile aria-pressed (the pre-highlighted default, §1.5)", () => {
    const { getByRole } = render(
      <FormatPicker targets={[tsv, csv]} selected={{ format: "tsv" }} onSelect={() => undefined} />,
    );
    expect(getByRole("button", { name: "TSV" }).getAttribute("aria-pressed")).toBe("true");
    expect(getByRole("button", { name: "CSV" }).getAttribute("aria-pressed")).toBe("false");
  });

  it("fires onSelect with the clicked tile's TargetId (the §5.8 re-plan trigger)", () => {
    const onSelect = vi.fn();
    const { getByRole } = render(
      <FormatPicker targets={[tsv, csv]} selected={{ format: "tsv" }} onSelect={onSelect} />,
    );
    fireEvent.click(getByRole("button", { name: "CSV" }));
    expect(onSelect).toHaveBeenCalledWith({ format: "csv" });
  });

  it("focuses the default (selected) tile on entry so a keyboard user is not stranded (§5.6.1)", () => {
    const { getByRole } = render(
      <FormatPicker targets={[tsv, csv]} selected={{ format: "tsv" }} onSelect={() => undefined} />,
    );
    expect(document.activeElement).toBe(getByRole("button", { name: "TSV" }));
  });

  it("renders the single-target slice offer (CSV→TSV) — one pre-selected tile", () => {
    const { getByRole } = render(
      <FormatPicker targets={[tsv]} selected={{ format: "tsv" }} onSelect={() => undefined} />,
    );
    expect(getByRole("button", { name: "TSV" }).getAttribute("aria-pressed")).toBe("true");
  });
});

describe("targetKey (§0.6 TargetId → stable key)", () => {
  it("namespaces format vs op so a format value can never collide with an op value", () => {
    expect(targetKey({ format: "tsv" })).toBe("format:tsv");
    expect(targetKey({ op: "toGif" })).toBe("op:toGif");
  });
});
