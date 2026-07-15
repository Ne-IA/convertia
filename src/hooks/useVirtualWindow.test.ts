import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { UIEvent } from "react";

import { useVirtualWindow, windowRange } from "./useVirtualWindow";

// §6.4.6 unit (G15): the §1.10 fixed-row-height windowing primitive (P3.55). The window MATH is pure
// (`windowRange`) so jsdom's absent layout is a non-issue — it is unit-tested directly across the clamps
// (empty, all-visible, scrolled, over-/under-scroll, overscan); the hook leg pins that `onScroll` re-windows.
// [Build-Session-Entscheidung: P3.55]
const OPTS = { rowHeight: 40, viewportHeight: 360, overscan: 3 } as const; // 9 visible rows + 3 overscan

const scrollEvent = (scrollTop: number): UIEvent<HTMLElement> =>
  ({ currentTarget: { scrollTop } }) as unknown as UIEvent<HTMLElement>;

describe("windowRange (pure §1.10 window math)", () => {
  it("is an empty window for an empty list", () => {
    expect(windowRange(0, { ...OPTS, itemCount: 0 })).toEqual({
      startIndex: 0,
      endIndex: 0,
      totalHeight: 0,
      offsetY: 0,
    });
  });

  it("renders every row when the whole list fits the viewport (small list)", () => {
    const range = windowRange(0, { ...OPTS, itemCount: 5 });
    expect(range).toEqual({ startIndex: 0, endIndex: 5, totalHeight: 200, offsetY: 0 });
  });

  it("windows to the top slice (+overscan below) at scrollTop 0 for a long list", () => {
    const range = windowRange(0, { ...OPTS, itemCount: 100 });
    // firstVisible 0, 9 visible + 3 overscan = [0, 12); spacer = 100*40; window at 0.
    expect(range).toEqual({ startIndex: 0, endIndex: 12, totalHeight: 4000, offsetY: 0 });
  });

  it("windows around the scroll offset (+overscan both sides) mid-list", () => {
    const range = windowRange(400, { ...OPTS, itemCount: 100 }); // scrolled to row 10
    // firstVisible 10 → start max(0, 10-3)=7; end min(100, 10+9+3)=22; offset 7*40=280.
    expect(range).toEqual({ startIndex: 7, endIndex: 22, totalHeight: 4000, offsetY: 280 });
  });

  it("clamps an over-scroll to the list end (never indexes past itemCount)", () => {
    const range = windowRange(9_999_999, { ...OPTS, itemCount: 100 });
    // clamped to totalHeight 4000 → firstVisible 100 → start 97; end min(100, 112)=100.
    expect(range).toEqual({ startIndex: 97, endIndex: 100, totalHeight: 4000, offsetY: 3880 });
  });

  it("clamps a negative scroll (bounce) to the top", () => {
    expect(windowRange(-50, { ...OPTS, itemCount: 100 }).startIndex).toBe(0);
  });

  it("honours a zero overscan (tight window, no padding rows)", () => {
    const range = windowRange(400, {
      rowHeight: 40,
      viewportHeight: 360,
      overscan: 0,
      itemCount: 100,
    });
    expect(range).toEqual({ startIndex: 10, endIndex: 19, totalHeight: 4000, offsetY: 400 });
  });

  it("defaults overscan to 3 when omitted", () => {
    const withDefault = windowRange(400, { rowHeight: 40, viewportHeight: 360, itemCount: 100 });
    expect(withDefault.startIndex).toBe(7);
  });
});

describe("useVirtualWindow (the scrollTop state over windowRange)", () => {
  it("starts at the top window and re-windows on scroll", () => {
    const { result } = renderHook(() => useVirtualWindow({ ...OPTS, itemCount: 100 }));
    expect(result.current.startIndex).toBe(0);
    expect(result.current.endIndex).toBe(12);
    act(() => {
      result.current.onScroll(scrollEvent(400));
    });
    expect(result.current.startIndex).toBe(7);
    expect(result.current.offsetY).toBe(280);
  });
});
