// src/components/FormatPicker.tsx — the §5.3 FormatPicker: the §5.2 Targets (state 4) target tiles (P3.56).
//
// The offered-target picker: renders the C3 `TargetOffer.targets` as selectable tiles with the one
// pre-highlighted default (§1.5), and fires `onSelect` on a tile click. Presentational + wired to the machine via
// `onSelect` (§5.3) — the parent TargetsScreen dispatches `selectTarget` + re-plans (C4). The SLICE renderer
// (P3.56): plain <button> tiles with a visual + `aria-pressed` selected marker; the full §5.6
// `radiogroup`/roving-tabindex/`aria-checked`, the §3.4 patent-gap disabled tiles, and the §2.9 lossy-note slot are
// P4.70.2 / P4.65 — which SUPERSEDE this slice FormatPicker per the P3↔P4 UI-seam model. For the CSV→TSV slice the
// offer is a single TSV target (which IS the default). Focus lands on the default tile on entry (§5.6.1) so a
// keyboard user is not stranded on <body> after the Confirm Continue unmounts. [Build-Session-Entscheidung: P3.56]
import { useEffect, useRef } from "react";

import type { Target, TargetId } from "../lib/ipc/commands";
import { ui } from "../strings/ui";

/** A stable string key for a §0.6 `TargetId` (`{ format }` | `{ op }`) — the React key AND the selected-tile
 *  comparison. Namespaced (`format:` / `op:`) so a format value can never collide with an op value.
 *  [Build-Session-Entscheidung: P3.56] */
export function targetKey(id: TargetId): string {
  return id.format !== undefined ? `format:${id.format}` : `op:${id.op}`;
}

export interface FormatPickerProps {
  /** The C3 offered targets + the pre-highlighted default (§1.5). */
  readonly targets: Target[];
  /** The currently-selected target (the offer's default until the user changes it). */
  readonly selected: TargetId;
  /** Fired with a tile's `TargetId` on click — the parent dispatches `selectTarget` + re-plans (C4, §5.8). */
  readonly onSelect: (target: TargetId) => void;
}

/** The §5.3 FormatPicker (slice renderer). [Build-Session-Entscheidung: P3.56] */
export function FormatPicker({ targets, selected, onSelect }: FormatPickerProps) {
  const selectedRef = useRef<HTMLButtonElement>(null);
  // §5.6.1: focus the default (selected) tile on ENTERING Targets (mount only) — the full radiogroup
  // roving-tabindex is P4.70.2. The effect runs once, so a later selection change does not re-steal focus.
  useEffect(() => {
    selectedRef.current?.focus();
  }, []);

  const selectedKey = targetKey(selected);
  return (
    <div className="flex flex-col gap-2">
      <h2 className="text-xl font-semibold text-text">{ui.formatpicker_heading}</h2>
      <div className="flex flex-wrap gap-3">
        {targets.map((target) => {
          const key = targetKey(target.id);
          const isSelected = key === selectedKey;
          return (
            <button
              key={key}
              ref={isSelected ? selectedRef : undefined}
              type="button"
              aria-pressed={isSelected}
              onClick={() => onSelect(target.id)}
              className={[
                "rounded-md border px-4 py-2 text-base",
                isSelected
                  ? "border-accent bg-accent text-accent-contrast"
                  : "border-border bg-surface text-text",
              ].join(" ")}
            >
              {target.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
