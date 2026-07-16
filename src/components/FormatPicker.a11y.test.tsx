import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 FormatPicker's own axe leg. Renders INTO a `<main>` (the App wraps
// state screens there) so the region best-practice rule is satisfied. The slice tiles are plain `aria-pressed`
// toggle buttons — axe-clean; the full §5.6 radiogroup/roving-tabindex is P4.70.2. [Build-Session-Entscheidung: P3.56]
import { FormatPicker } from "./FormatPicker";
import type { Target } from "../lib/ipc/commands";

const tsv: Target = {
  id: { format: "tsv" },
  label: "TSV",
  lossy: null,
  availability: "available",
  options: [],
};

afterEach(cleanup);

describe("FormatPicker — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations", async () => {
    const { container } = render(
      <main>
        <FormatPicker targets={[tsv]} selected={{ format: "tsv" }} onSelect={() => undefined} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
