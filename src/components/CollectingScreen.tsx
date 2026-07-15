// src/components/CollectingScreen.tsx — the §5.2 Collecting (state 2) minimal scanning indicator (P3.55).
//
// The brief collect-step screen the §5.8 nudge consumption transits (Idle → Collecting → Confirm): a
// `role="status"` region showing the throttled `onScan` count ("Scanning… N files so far", or the
// indeterminate "Looking at your files…" until a count arrives, §5.2) — the §5.6.1 state-2 non-orphaned SR
// landing element — plus the §5.2/§5.10 cancel-collect affordance (Esc OR the button) backed by C13
// `cancel_ingest`, which discards the partial unfrozen set and returns to Idle (§1.1). Kept MINIMAL: this box
// makes Collecting reachable (the consumption seam drives through it) and renders it non-blank; the P4.70
// focus-on-entry to Idle/Collecting refines it. [Build-Session-Entscheidung: P3.55]
import { useEffect } from "react";

import { cancelIntakeCollect } from "../lib/ipc/events";
import type { CollectingId } from "../lib/ipc/commands";
import { formatScanStatus } from "../strings/format";
import { ui } from "../strings/ui";

export interface CollectingScreenProps {
  /** The in-flight walk's §0.4.4 ingest-cancel handle (from the §5.2 `collecting` state) — C13's target. */
  readonly collectingId: CollectingId;
  /** The throttled `onScan` live count, or `null` for the indeterminate "Looking at your files…" fallback. */
  readonly scanned: number | null;
}

/**
 * The §5.2 Collecting screen. Esc or the Cancel button fires C13 `cancel_ingest` (via
 * {@link cancelIntakeCollect}), which trips the ingest token + advances the machine to Idle. Fire-and-forget:
 * a C13 rejection surfaces through the §7.5.1 global frontend-error bridge (like every intake trigger, §5.4).
 * [Build-Session-Entscheidung: P3.55]
 */
export function CollectingScreen({ collectingId, scanned }: CollectingScreenProps) {
  // §5.10: Esc cancels an in-flight collect (backs the cancel-collect control for a large recursive walk, §1.10).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        void cancelIntakeCollect(collectingId);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [collectingId]);

  return (
    <div className="flex flex-col items-center gap-4 p-10">
      <p role="status" className="text-base text-text">
        {formatScanStatus(scanned)}
      </p>
      <button
        type="button"
        onClick={() => void cancelIntakeCollect(collectingId)}
        className="text-accent underline"
      >
        {ui.collecting_cancel}
      </button>
    </div>
  );
}
