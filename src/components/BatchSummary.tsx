// src/components/BatchSummary.tsx — the §5.3 BatchSummary: the §5.2 Confirm-gate (state 3) card (P3.55).
//
// The mandatory pre-convert gate's summary card: the detected-format + count line ("48 CSV files", §1.4) and,
// when any item was skipped at the §1.1 freeze, the passive one-line tally ("3 files weren't recognized and
// will be skipped", §5.2) — never blocks confirm, never silent (§1.4). It owns ONLY the tally count `M` and
// the §5.6 assertive announcement; the per-item skipped DETAIL is the single-owner §5.3 FileList's (below the
// ConfirmScreen's disclosure), never duplicated here. Presentational + display-only (§5.3): it renders lossy
// display strings, never a re-submittable path (the wire carries no path — the 2026-07-06 core-owned-paths
// ruling). [Build-Session-Entscheidung: P3.55]
import { useEffect } from "react";

import { announce } from "../a11y/announcer";
import type { UserFacingFormat } from "../lib/ipc/commands";
import { formatConfirmAnnouncement, formatConfirmCount, formatSkipTally } from "../strings/format";

export interface BatchSummaryProps {
  /** The §1.4 eligible count (`CollectedSet::Single.count`) — "48 CSV files". */
  readonly count: number;
  /** The §0.6 detected source format — rendered uppercased (§5.7 formatter). */
  readonly format: UserFacingFormat;
  /** How many items were skipped at the freeze (`skipped.length`) — drives the passive tally + the announced
   *  skip half; `0` renders no tally (the common clean-drop case). */
  readonly skippedCount: number;
}

/**
 * The §5.3 confirm-gate summary card. On mount (entering state 3) it announces the collected summary + skip
 * tally ASSERTIVELY (§5.6/§5.6.1 — the Confirm gate is a required decision point). The announcement is built
 * from the SAME `ui` templates as the visible count/tally, so the SR line and the visible line never diverge
 * (§5.6 "one source, no divergence"). [Build-Session-Entscheidung: P3.55]
 */
export function BatchSummary({ count, format, skippedCount }: BatchSummaryProps) {
  useEffect(() => {
    announce(formatConfirmAnnouncement(count, format, skippedCount), "assertive");
  }, [count, format, skippedCount]);

  return (
    <div className="flex flex-col gap-1">
      <h2 className="text-xl font-semibold text-text">{formatConfirmCount(count, format)}</h2>
      {skippedCount > 0 ? (
        <p className="text-sm text-text-muted">{formatSkipTally(skippedCount)}</p>
      ) : null}
    </div>
  );
}
