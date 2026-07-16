import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 DestinationBar's own axe leg. Renders INTO a `<main>` (the App wraps
// state screens there). Covers both the clean case and the up-front-fail disabled-Convert + note case (jsdom axe
// checks ARIA/roles, not contrast — contrast is the G33b live-WebView leg). [Build-Session-Entscheidung: P3.56]
import { DestinationBar } from "./DestinationBar";
import type { OutputPlanPreview } from "../lib/ipc/commands";

const basePreview: OutputPlanPreview = {
  set: "cs1",
  finalDirDisplay: "/drop",
  diverted: null,
  rerun: null,
  preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
};

afterEach(cleanup);

describe("DestinationBar — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations (Convert enabled, no notes)", async () => {
    const { container } = render(
      <main>
        <DestinationBar
          preview={basePreview}
          persistedFallback={false}
          onChangeDestination={() => undefined}
          onConvert={() => undefined}
        />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders with no axe violations (fallback note + divert + Convert disabled; disable-only, no §2.8 note in P3)", async () => {
    const preview: OutputPlanPreview = {
      ...basePreview,
      diverted: "ephemeral",
      preflight: { ...basePreview.preflight, upFrontFail: "outOfDisk" },
    };
    const { container } = render(
      <main>
        <DestinationBar
          preview={preview}
          persistedFallback={true}
          onChangeDestination={() => undefined}
          onConvert={() => undefined}
        />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
