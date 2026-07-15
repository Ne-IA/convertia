// src/components/FileList.tsx — the §5.3 FileList: the Confirm-gate (state 3) expandable per-item detail (P3.55).
//
// The SINGLE owner of the confirm-gate per-item detail (§5.3): a "Show N files" disclosure (§5.10, collapsed by
// default) that reveals every collected item — eligible rows plain, skipped rows visually marked with their
// reason (§5.3) so a bad item is NEVER silently dropped (§1.4). Rows are lossy DISPLAY strings, never
// re-submittable paths (the wire carries no path — the 2026-07-06 core-owned-paths ruling); read-only in v1
// (no per-item target / no per-item deselect, §5.3). The list is VIRTUALISED (§1.10) via the shared
// {@link useVirtualWindow} fixed-row-height windowing hook (the 1793e20 ruling: a hand-rolled hook, no new JS
// dependency). §5.6 "never focus a not-yet-mounted virtual row" holds by construction: the rows are
// non-focusable in v1, so only the disclosure button + the scroll container take focus. [Build-Session-Entscheidung: P3.55]
import { useId, useMemo, useState } from "react";

import { useVirtualWindow } from "../hooks/useVirtualWindow";
import type { ItemId, SkippedItem } from "../lib/ipc/commands";
import { formatDisclosure, formatSkipRow } from "../strings/format";
import { ui } from "../strings/ui";

/** Fixed row height in px — MUST match the row `h-10` class so the §1.10 windowing math indexes correctly. */
const ROW_HEIGHT = 40;
/** Fixed scroll-viewport height in px — MUST match the container `h-[360px]` class (≈9 rows before scroll). */
const VIEWPORT_HEIGHT = 360;

/** The §5.3 eligible-row display fields — the §0.6 `DroppedItem` DISPLAY subset the FileList renders (a full
 *  `DroppedItem` is structurally assignable, so `CollectedSet::Single.items` passes as-is). Narrowed to what is
 *  rendered so no re-submittable path field even reaches this presentational component. */
export interface FileListItem {
  /** The §0.6 invariant-6 freeze-assigned id — the React key (display-only). */
  readonly item: ItemId;
  /** The core-produced lossy display basename (§2.10.1), never a re-submittable path. */
  readonly displayName: string;
  /** The folder-drop root-relative subpath preview, or `null` for a top-level item (§2.7). */
  readonly relPathDisplay: string | null;
}

/** One rendered row — an eligible display item or a skipped `SkippedItem` (§0.6). The two id spaces are
 *  id-disjoint (§0.6 invariant 6); the `e`/`s` key prefix keeps React keys unique regardless. */
type Row =
  | { readonly kind: "eligible"; readonly key: string; readonly item: FileListItem }
  | { readonly kind: "skipped"; readonly key: string; readonly item: SkippedItem };

export interface FileListProps {
  /** The §1.4 eligible items (`CollectedSet::Single.items`) — display name + optional folder-relative subpath. */
  readonly items: readonly FileListItem[];
  /** The §1.4 skipped items (`CollectedSet::Single.skipped`) — source display + §2.8 reason, rendered marked. */
  readonly skipped: readonly SkippedItem[];
}

/**
 * The §5.3 FileList. Collapsed to a "Show N files" disclosure (N = total listed rows); expanding reveals the
 * virtualised eligible-then-skipped list. [Build-Session-Entscheidung: P3.55]
 */
export function FileList({ items, skipped }: FileListProps) {
  const [expanded, setExpanded] = useState(false);
  const listId = useId();

  // Memoised so the O(n) build + allocation (n up to thousands, §1.10) is confined to a prop change, not
  // re-run on every scroll tick (`useVirtualWindow`'s scrollTop state re-renders per scroll) — the whole point
  // of the windowing is a cheap re-render (Opus review, P3.55). `items`/`skipped` are the frozen-set refs,
  // stable across the Confirm gate. [Build-Session-Entscheidung: P3.55]
  const rows: Row[] = useMemo(
    () => [
      ...items.map((item) => ({ kind: "eligible" as const, key: `e${item.item}`, item })),
      ...skipped.map((item) => ({ kind: "skipped" as const, key: `s${item.item}`, item })),
    ],
    [items, skipped],
  );
  const total = rows.length;

  const { startIndex, endIndex, totalHeight, offsetY, onScroll } = useVirtualWindow({
    itemCount: total,
    rowHeight: ROW_HEIGHT,
    viewportHeight: VIEWPORT_HEIGHT,
  });
  const windowed = rows.slice(startIndex, endIndex);

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={expanded ? listId : undefined}
        onClick={() => setExpanded((open) => !open)}
        className="self-start text-sm text-accent underline"
      >
        {formatDisclosure(total, expanded)}
      </button>
      {expanded ? (
        <div
          id={listId}
          onScroll={onScroll}
          // §5.6: the rows are non-focusable (read-only, v1), so the scroll region itself must be
          // keyboard-focusable (tabIndex 0) + named — a keyboard/AT user Tabs here and arrow/Page-scrolls to
          // the rows past the first window (else a large batch's later rows are unreachable — Sonnet review).
          // [Build-Session-Entscheidung: P3.55]
          tabIndex={0}
          role="group"
          aria-label={ui.filelist_region_label}
          className="h-[360px] overflow-y-auto rounded-md border border-border bg-surface"
        >
          {/* [Build-Session-Entscheidung: P3.55] The rail height + the window `translateY` are the ONLY dynamic
              §1.10 virtualisation values — a computed px offset has NO static Tailwind class, so inline style is
              required here (the fixed viewport/row heights stay classes: h-[360px] / h-10). */}
          <div className="relative" style={{ height: `${totalHeight}px` }}>
            <div
              className="absolute top-0 w-full"
              style={{ transform: `translateY(${offsetY}px)` }}
            >
              {windowed.map((row) =>
                row.kind === "eligible" ? (
                  <div key={row.key} className="flex h-10 items-center gap-2 px-3">
                    <span className="min-w-0 flex-1 truncate text-base text-text">
                      {row.item.displayName}
                    </span>
                    {row.item.relPathDisplay !== null ? (
                      <span className="min-w-0 flex-1 truncate text-sm text-text-muted">
                        {row.item.relPathDisplay}
                      </span>
                    ) : null}
                  </div>
                ) : (
                  <div
                    key={row.key}
                    data-skipped="true"
                    className="flex h-10 items-center gap-2 px-3"
                  >
                    <span className="min-w-0 flex-1 truncate text-base text-text-muted line-through">
                      {row.item.sourceDisplay}
                    </span>
                    <span className="shrink-0 text-sm text-text-muted">
                      {formatSkipRow(row.item.reason, row.item.detectedDisplay)}
                    </span>
                  </div>
                ),
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
