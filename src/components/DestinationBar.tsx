// src/components/DestinationBar.tsx — the §5.3 DestinationBar: the §5.2 Destination (state 5) preview + actions (P3.56).
//
// Always visible before Convert (§5.2 state 5): the "will save to …" line (the C4 plan's `finalDirDisplay`,
// §1.8/§2.7); the §5.8:926 passive FALLBACK note when the persisted-destination hand-off (C14) fell back to
// beside-source (`persistedFallback`, §5.7:825 chrome); the per-location divert note when the plan diverted
// (§2.7.2); the Change-destination button (drives C2b `pick_destination` → C5 `set_destination`, §5.4); and the
// Convert button — DISABLED (no Note) when the C4 `preflight.upFrontFail` is `Some(kind)` (the SSOT "fails fast up
// front", §1.10/§5.3). Presentational + wired to the machine via `onChangeDestination`/`onConvert` (§5.3).
//
// **Up-front-fail is DISABLE-ONLY in P3 (Co-Pilot ruling item 1, 7f73553):** the verbatim §2.8 string is not
// honestly buildable in P3 (the wire carries only the KIND; the §2.8.2 rows are item-scoped vs. the whole-batch
// verdict) — the passive §2.8 `Note` rides P4.69 (UI) + P4.72 (backend-rendered wire text), the P4.65 class. For
// the CSV→TSV slice `preflight.upFrontFail` is always `null` anyway (the P3.49 §1.10-seam stub; the real verdict is
// P4.72). The full §5.6 keyboard/focus a11y (Convert/Change roving + describedby) is P4.70.3.
// [Build-Session-Entscheidung: P3.56]
import type { OutputPlanPreview } from "../lib/ipc/commands";
import { divertNote, formatWillSaveTo } from "../strings/format";
import { ui } from "../strings/ui";

export interface DestinationBarProps {
  /** The C4 plan preview — the "will save to …" dir, the §2.7.2 divert, and the §1.10 preflight verdict. */
  readonly preview: OutputPlanPreview;
  /** §5.8:926 — the persisted-destination re-validation FALLBACK fact (from the C14 hand-off via `Planned`);
   *  `true` renders the passive §5.7:825 chrome fallback note (surfaced even when beside-source is writable). */
  readonly persistedFallback: boolean;
  /** Fired on the Change-destination button — the parent drives C2b `pick_destination` → C5 `set_destination`. */
  readonly onChangeDestination: () => void;
  /** Fired on the Convert button (enabled only when not up-front doomed) — the parent fires C6 / branches to Rerun. */
  readonly onConvert: () => void;
}

/** The §5.3 DestinationBar (slice renderer). [Build-Session-Entscheidung: P3.56] */
export function DestinationBar({
  preview,
  persistedFallback,
  onChangeDestination,
  onConvert,
}: DestinationBarProps) {
  // §1.10/§5.3: a whole-batch up-front doom DISABLES Convert (the §2.8-string Note rides P4.69/P4.72, ruling item 1).
  const convertDisabled = preview.preflight.upFrontFail !== null;
  return (
    <div className="flex flex-col gap-2">
      <p className="text-base text-text">{formatWillSaveTo(preview.finalDirDisplay)}</p>
      {persistedFallback ? (
        <p className="text-sm text-text-muted">{ui.destination_persisted_fallback}</p>
      ) : null}
      {preview.diverted !== null ? (
        <p className="text-sm text-text-muted">{divertNote(preview.diverted)}</p>
      ) : null}
      <div className="flex gap-3">
        <button
          type="button"
          onClick={onConvert}
          disabled={convertDisabled}
          className="rounded-md bg-accent px-4 py-2 text-base font-medium text-accent-contrast disabled:cursor-not-allowed disabled:opacity-50"
        >
          {ui.destination_convert}
        </button>
        <button
          type="button"
          onClick={onChangeDestination}
          className="rounded-md border border-border px-4 py-2 text-base text-text"
        >
          {ui.destination_change}
        </button>
      </div>
    </div>
  );
}
