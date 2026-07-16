import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 DestinationBar (state 5) — the will-save-to line, the §5.8:926 persisted-fallback
// note, the §2.7.2 divert note, the Change/Convert callbacks, and the §1.10 up-front-fail DISABLE (disable-only in
// P3; the §2.8 Note rides P4.69/P4.72). Pure presentational (props only), so no mocks. [Build-Session-Entscheidung: P3.56]
import { DestinationBar } from "./DestinationBar";
import type { OutputPlanPreview } from "../lib/ipc/commands";
import { ui } from "../strings/ui";

const basePreview: OutputPlanPreview = {
  set: "cs1",
  finalDirDisplay: "/drop",
  diverted: null,
  rerun: null,
  preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
};

afterEach(cleanup);

describe("DestinationBar — §5.3 destination preview + actions (state 5)", () => {
  it("renders the will-save-to line from the C4 plan's finalDirDisplay (§1.8/§2.7)", () => {
    const { getByText } = render(
      <DestinationBar
        preview={basePreview}
        persistedFallback={false}
        onChangeDestination={() => undefined}
        onConvert={() => undefined}
      />,
    );
    expect(getByText("Will save to /drop")).not.toBeNull();
  });

  it("Convert + Change fire their callbacks (never a dead button)", () => {
    const onConvert = vi.fn();
    const onChangeDestination = vi.fn();
    const { getByRole } = render(
      <DestinationBar
        preview={basePreview}
        persistedFallback={false}
        onChangeDestination={onChangeDestination}
        onConvert={onConvert}
      />,
    );
    fireEvent.click(getByRole("button", { name: "Convert" }));
    fireEvent.click(getByRole("button", { name: "Change destination" }));
    expect(onConvert).toHaveBeenCalledTimes(1);
    expect(onChangeDestination).toHaveBeenCalledTimes(1);
  });

  // [Test-Change: P3.56 — old-obsolete+new-correct, §5.3] the up-front-fail §2.8 Note is REMOVED from the P3 slice
  // (Co-Pilot ruling item 1 = A, 7f73553: the verbatim §2.8 string is not honestly buildable in P3 — the wire
  // carries only the KIND; it rides P4.69 UI + P4.72 wire-text). P3 surfaces the up-front-fail as DISABLE-ONLY, so
  // the two `ui.preflight_too_big` note-presence/absence assertions are obsolete; the disable assertions stand.
  it("Convert is ENABLED when preflight.upFrontFail is null (the P3 slice case — upFrontFail is always null here)", () => {
    const { getByRole } = render(
      <DestinationBar
        preview={basePreview}
        persistedFallback={false}
        onChangeDestination={() => undefined}
        onConvert={() => undefined}
      />,
    );
    expect((getByRole("button", { name: "Convert" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("Convert is DISABLED (disable-only in P3; the §2.8 Note rides P4.69/P4.72) when preflight.upFrontFail is Some", () => {
    const onConvert = vi.fn();
    const preview: OutputPlanPreview = {
      ...basePreview,
      preflight: { ...basePreview.preflight, upFrontFail: "tooBig" },
    };
    const { getByRole } = render(
      <DestinationBar
        preview={preview}
        persistedFallback={false}
        onChangeDestination={() => undefined}
        onConvert={onConvert}
      />,
    );
    const convert = getByRole("button", { name: "Convert" }) as HTMLButtonElement;
    expect(convert.disabled).toBe(true);
    fireEvent.click(convert);
    expect(onConvert).not.toHaveBeenCalled(); // a disabled button fires nothing
  });

  it("renders the §2.7.2 per-location divert note when the plan diverted", () => {
    const preview: OutputPlanPreview = { ...basePreview, diverted: "unwritable" };
    const { getByText } = render(
      <DestinationBar
        preview={preview}
        persistedFallback={false}
        onChangeDestination={() => undefined}
        onConvert={() => undefined}
      />,
    );
    expect(getByText(ui.destination_divert_unwritable)).not.toBeNull();
  });

  it("renders the §5.8:926 passive fallback note when persistedFallback is true (§5.7:825 chrome)", () => {
    const { getByText } = render(
      <DestinationBar
        preview={basePreview}
        persistedFallback={true}
        onChangeDestination={() => undefined}
        onConvert={() => undefined}
      />,
    );
    expect(getByText(ui.destination_persisted_fallback)).not.toBeNull();
  });

  it("shows NO fallback note when persistedFallback is false (the common case)", () => {
    const { queryByText } = render(
      <DestinationBar
        preview={basePreview}
        persistedFallback={false}
        onChangeDestination={() => undefined}
        onConvert={() => undefined}
      />,
    );
    expect(queryByText(ui.destination_persisted_fallback)).toBeNull();
  });
});
