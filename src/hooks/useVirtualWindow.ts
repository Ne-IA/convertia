// src/hooks/useVirtualWindow.ts — the §1.10 fixed-row-height list windowing primitive (P3.55, Co-Pilot ruling
// 1793e20).
//
// A hand-rolled fixed-row-height virtualisation hook: from a scroll offset + a fixed row height + a fixed
// viewport height it computes the visible index window (with overscan), the full spacer height, and the window
// offset. The §5.3 FileList is the first consumer; it is authored as a SHARED primitive because §1.10 delegates
// list virtualisation to §5 ("the UI list is virtualized — §5 owns the virtualization component") and the
// §5.3 FileList / ProgressList (P4.66) / Summary results list rows all carry the same fixed-height,
// read-only-in-v1 shape. NO new JS dependency (the 1793e20 ruling: a dep = the dep↔floor two-box split + an
// L(-1) Co-Pilot floor leg, disproportionate for a fixed-height list); a genuine insufficiency (dynamic row
// heights, scroll anchoring) ESCALATES rather than quietly pulling a dep.
//
// The window MATH is the pure {@link windowRange} (unit-tested without a DOM); the hook only adds the
// scrollTop state + the `onScroll` handler over it. §5.6 "never focus a not-yet-mounted virtual row" holds by
// construction for the FileList (its rows are read-only / non-focusable in v1, §5.3); a future consumer with
// focusable rows uses `startIndex`/`endIndex` to scroll a target row into range before focusing it.
// [Build-Session-Entscheidung: P3.55]
import { useCallback, useState, type UIEvent } from "react";

/** The fixed inputs for the window computation — a fixed row height + a fixed viewport height (both in px). */
export interface VirtualWindowOptions {
  /** Total number of rows in the full list. */
  readonly itemCount: number;
  /** Fixed height of every row, in px (the FileList rows are uniform). */
  readonly rowHeight: number;
  /** Fixed height of the scroll viewport, in px. */
  readonly viewportHeight: number;
  /** Extra rows rendered above + below the visible range so a fast scroll shows no blank gap. Default 3. */
  readonly overscan?: number;
}

/** The computed visible window: the rendered slice `[startIndex, endIndex)`, the total spacer height (so the
 *  scrollbar reflects the FULL list), and the window offset (`translateY` for the rendered rows). */
export interface VirtualWindowRange {
  /** First rendered row index (inclusive). */
  readonly startIndex: number;
  /** One past the last rendered row index (exclusive) — the slice is rows `[startIndex, endIndex)`. */
  readonly endIndex: number;
  /** The full list height in px (`itemCount * rowHeight`) — the scroll spacer, so the scrollbar is honest. */
  readonly totalHeight: number;
  /** The rendered window's vertical offset in px (`startIndex * rowHeight`). */
  readonly offsetY: number;
}

/** Pure window math — given a scroll offset, which row slice is visible (+ overscan), the spacer height, and
 *  the window offset. Clamped so an empty list, a viewport taller than the list, or an over-scroll never
 *  produce an out-of-range slice. No DOM, no React — unit-tested directly. */
export function windowRange(
  scrollTop: number,
  { itemCount, rowHeight, viewportHeight, overscan = 3 }: VirtualWindowOptions,
): VirtualWindowRange {
  const totalHeight = itemCount * rowHeight;
  // Clamp scrollTop into [0, totalHeight] so a bounce/over-scroll never indexes past the list.
  const clampedTop = Math.min(Math.max(scrollTop, 0), Math.max(totalHeight, 0));
  const firstVisible = Math.floor(clampedTop / rowHeight);
  const visibleRows = Math.ceil(viewportHeight / rowHeight);
  const startIndex = Math.max(0, firstVisible - overscan);
  const endIndex = Math.min(itemCount, firstVisible + visibleRows + overscan);
  return { startIndex, endIndex, totalHeight, offsetY: startIndex * rowHeight };
}

/** The §5.3 FileList windowing hook: {@link windowRange} over an internal `scrollTop` the returned `onScroll`
 *  updates. Bind `onScroll` to the fixed-height scroll container; render the spacer at `totalHeight`, the row
 *  window `[startIndex, endIndex)` translated by `offsetY`. [Build-Session-Entscheidung: P3.55] */
export function useVirtualWindow(
  options: VirtualWindowOptions,
): VirtualWindowRange & { readonly onScroll: (event: UIEvent<HTMLElement>) => void } {
  const [scrollTop, setScrollTop] = useState(0);
  const onScroll = useCallback((event: UIEvent<HTMLElement>) => {
    setScrollTop(event.currentTarget.scrollTop);
  }, []);
  return { ...windowRange(scrollTop, options), onScroll };
}
