// src/components/SummaryScreen.tsx — the §5.2 Summary screen (state 8) (P3.59).
//
// Composes the §5.3 ResultSummary (the §1.12 per-item outcome) + OpenActions (the §7.7 open-folder shell-out)
// and OWNS the state-8 out-transition (the P3 screen-box wiring model — a rendered action MUST fire its
// command): "Convert more" → the machine's `convertMore` Msg → `Idle` (§5.2 row 8).
//
// The §1.12 output→SOURCE map is resolved here: `RunResult.items[]` carries only the `ItemId` anchor (P3.76
// retired `source: PathBuf` under the 2026-07-06 core-owned-paths ruling), so the source DISPLAY comes from the
// frozen `CollectedSet` the machine threads Confirm → Targets → Converting → Summary. `items` + `skipped` span
// the whole §0.6-invariant-6 id space, so every projected row — including a pre-flight skip, which emits no
// `ItemStarted` and therefore has no live progress row — resolves to a name.
//
// SLICE SCOPE (P3.59): §5.3 [DECIDED] "Summary renders no DropZone" — a drop in Summary still starts a new batch
// via the window-global core-side drop path (§5.4), and the keyboard equivalent is the deliberate two-step
// Ctrl/⌘+N ("Convert more" → Idle) then Ctrl/⌘+O, so this screen binds no single-chord picker. The §5.10
// Ctrl/⌘+N accelerator itself + the §5.6 priority focus-on-entry (first Failed row → else the OpenActions
// primary → else the banner action) + the assertive outcome announcement are P4.70.3/P4.75. Visual polish is
// P8. The §2.8.2 batch-level summary line SHIPS here (the 2026-07-16 P3.59 ruling wired the core's
// `batch_summary_line` onto `RunResult.summaryLineDisplay`); ResultSummary renders it verbatim.
// [Build-Session-Entscheidung: P3.59]
import { useCallback, useMemo } from "react";

import type { ItemId } from "../lib/ipc/commands";
import { useAppStore, type State } from "../state/store";
import { ui } from "../strings/ui";

import { OpenActions } from "./OpenActions";
import { ResultSummary } from "./ResultSummary";

type SummaryState = State & { tag: "summary" };

export interface SummaryScreenProps {
  /** The terminal §1.12 `RunResult` from the machine's `summary` state. */
  readonly result: SummaryState["result"];
  /** The frozen §1.4 set threaded through Converting — the source-name side of the §1.12 output→source map. */
  readonly set: SummaryState["set"];
}

/** The §5.2 Summary screen (state 8). [Build-Session-Entscheidung: P3.59] */
export function SummaryScreen({ result, set }: SummaryScreenProps) {
  const dispatch = useAppStore((state) => state.dispatch);

  // The §1.12 `ItemId` → source-display index over the WHOLE frozen id space: the eligible `DroppedItem`s
  // (`displayName`) + the pre-flight `SkippedItem`s (`sourceDisplay`) — id-disjoint views of one id space
  // (§0.6 invariant 6). Memoised on the frozen set, which never changes within a Summary.
  const sources = useMemo<ReadonlyMap<ItemId, string>>(() => {
    const index = new Map<ItemId, string>();
    for (const item of set.items) {
      index.set(item.item, item.displayName);
    }
    for (const skipped of set.skipped) {
      index.set(skipped.item, skipped.sourceDisplay);
    }
    return index;
  }, [set]);

  // §5.2 row 8: "Convert more" → Idle. The machine's `convertMore` arm discards the run; the next run's
  // `runStarted` clears the store's live progress (§5.8), so no prior-run rows survive into a fresh batch.
  const onConvertMore = useCallback((): void => {
    dispatch({ type: "convertMore" });
  }, [dispatch]);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      <h2 className="text-xl font-semibold text-text">{ui.summary_heading}</h2>
      <ResultSummary result={result} sources={sources} />
      <OpenActions
        commonRootDisplay={result.commonRootDisplay}
        divertRootDisplay={result.divertRootDisplay}
      />
      <button
        type="button"
        onClick={onConvertMore}
        className="self-start rounded-md border border-border px-4 py-2 text-base text-text"
      >
        {ui.summary_convert_more}
      </button>
    </div>
  );
}
