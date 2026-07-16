// src/components/ProgressList.tsx — the §5.3 ProgressList: per-item rows + the aggregate batch bar (P3.58).
//
// Renders the §5.8 live progress the store reduces from the §0.4.2 `ConversionEvent` stream: an aggregate
// `BatchProgress` bar (`done`/`total`, §1.11) + a per-item row for each §0.6 `ItemId` — the source display, a
// **real determinate** progress bar while `running` (never a bare spinner, §1.11), and the terminal status once
// the item finishes (`Succeeded`/`Failed`/`Cancelled`/`Skipped`, §1.11); a `Failed` row additionally shows the
// item's verbatim §2.8 reason (the store's `ItemRow.reason` = the `IpcError.message`, §5.7 §02-owned).
//
// The bars are native **`<progress>`** elements — implicit `role="progressbar"` + `aria-valuenow` for free, a
// determinate `value`/`max` (or an indeterminate `<progress>` for the §1.11 `null`-fraction LibreOffice case),
// and NO inline `style=` for the dynamic width (the anti-pattern). Presentational + wired via props (§5.3): the
// ConvertingScreen reads the store and passes `rows` + `batchProgress`. SLICE SCOPE (P3.58): the full §5.6
// structural a11y (explicit `role=progressbar` + `aria-valuemin/max/now` refinement, indeterminate-row
// `aria-busy`, virtualisation for 1000-row batches, §1.10) is P4.70.3; visual styling/polish is P8. The slice's
// native `<progress>` is already progressbar-accessible + labelled by its row's source display.
// [Build-Session-Entscheidung: P3.58]
import { useId } from "react";

import { formatBatchProgress } from "../strings/format";
import { ui } from "../strings/ui";
import type { ItemId } from "../lib/ipc/commands";
import type { ItemRow, ItemRowStatus } from "../state/store";

export interface ProgressListProps {
  /** The §5.8 live per-item progress map, keyed by §0.6 `ItemId` (the store's reduced `ItemRow`s). */
  readonly rows: Readonly<Record<ItemId, ItemRow>>;
  /** The §1.11 aggregate `done`/`total`, or `null` before the first `BatchProgress` tick. */
  readonly batchProgress: { readonly done: number; readonly total: number } | null;
}

/** The per-row status chrome label (§5.7) for an `ItemRowStatus` — exhaustive over the five states.
 *  [Build-Session-Entscheidung: P3.58] */
function statusLabel(status: ItemRowStatus): string {
  switch (status) {
    case "running":
      return ui.converting_status_running;
    case "succeeded":
      return ui.converting_status_succeeded;
    case "failed":
      return ui.converting_status_failed;
    case "cancelled":
      return ui.converting_status_cancelled;
    case "skipped":
      return ui.converting_status_skipped;
    default: {
      // Exhaustiveness guard: a new `ItemRowStatus` reaching here fails to compile (`never`). Unreachable.
      const exhaustive: never = status;
      throw new Error(`unhandled ItemRowStatus: ${String(exhaustive)}`);
    }
  }
}

/** The §5.3 ProgressList (slice renderer). [Build-Session-Entscheidung: P3.58] */
export function ProgressList({ rows, batchProgress }: ProgressListProps) {
  const labelBase = useId();
  const aggregateId = useId();
  // Stable numeric order (§1.9 item indices), so the rows never re-shuffle as ticks arrive.
  const ordered = Object.entries(rows)
    .map(([key, row]) => ({ itemId: Number(key), row }))
    .sort((a, b) => a.itemId - b.itemId);

  return (
    <div className="flex flex-col gap-4">
      {batchProgress !== null && batchProgress.total > 0 ? (
        <div className="flex flex-col gap-1">
          <p id={aggregateId} className="text-base text-text">
            {formatBatchProgress(batchProgress.done, batchProgress.total)}
          </p>
          <progress
            aria-labelledby={aggregateId}
            max={batchProgress.total}
            value={batchProgress.done}
            className="w-full"
          />
        </div>
      ) : null}
      <ul className="flex flex-col gap-3">
        {ordered.map(({ itemId, row }) => {
          const labelId = `${labelBase}-${itemId}`;
          return (
            <li key={itemId} className="flex flex-col gap-1">
              <span id={labelId} className="text-base text-text">
                {row.sourceDisplay}
              </span>
              <span className="text-sm text-text-muted">{statusLabel(row.status)}</span>
              {row.status === "running" ? (
                row.fraction !== null ? (
                  <progress
                    aria-labelledby={labelId}
                    max={1}
                    value={row.fraction}
                    className="w-full"
                  />
                ) : (
                  // §1.11 indeterminate stage (LibreOffice) — a value-less `<progress>` (the P4.70.3 staged-bar
                  // + aria-busy refinement rides that box); not reached by the CSV→TSV slice's real fraction.
                  <progress aria-labelledby={labelId} className="w-full" />
                )
              ) : null}
              {row.status === "failed" && row.reason !== null ? (
                <p className="text-sm text-danger">{row.reason}</p>
              ) : null}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
