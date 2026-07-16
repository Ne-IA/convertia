// src/components/OpenActions.tsx — the §5.3 OpenActions: the §7.7 open-folder shell-out(s) (P3.59).
//
// Summary-ONLY (§5.3 [DECIDED]: state 8, never mid-run — during `Converting` the run's results are incomplete
// and the §7.7.3 `RunResultStore` resolution set is not final, so a mid-run C9 could open a folder of
// half-written outputs; the core refuses it anyway). Presentational + wired via props (§5.3): the SummaryScreen
// reads the machine state and passes the two §1.12 display roots.
//
// SPLIT-DIVERT (§5.3/§7.7.1 [DECIDED]): when `divertRootDisplay` is present (any item diverted, §2.7.3) render
// BOTH a common-root button ("Open source folder") and a divert button ("Open saved-to folder") plus the
// connector line explaining the split — a SINGLE button would strand a user whose files diverted. When absent,
// render only the common-root button, labelled "Open folder".
//
// The WebView fires C9 by ID, never by path (§7.7.2, the 2026-07-06 core-owned-paths ruling): the core resolves
// `OpenTarget` against `State<RunResultStore>` to its own recorded `PathBuf`. The `*Display` strings label the
// buttons; they are lossy display forms (§2.10.1) and are never re-submitted as a path.
//
// SLICE SCOPE (P3.59): "Open file" (`OpenTarget::Item(ItemId)`, the single-output run) + the §5.10
// Ctrl/⌘+Shift+F / Ctrl/⌘+Shift+Enter accelerators + the §5.6 two-button keyboard/focus order are P4.68/P4.70.3,
// which SUPERSEDE this slice renderer. Visual polish is P8. [Build-Session-Entscheidung: P3.59]
import { useCallback, useId } from "react";

import { openResultTarget } from "../lib/ipc/events";
import { formatSavedToConnector } from "../strings/format";
import { ui } from "../strings/ui";

export interface OpenActionsProps {
  /** §1.12 `RunResult.commonRootDisplay` — the beside-source open-folder button's LABEL (display-only, §2.10.1). */
  readonly commonRootDisplay: string;
  /** §1.12 `RunResult.divertRootDisplay` — `string` when ANY item diverted (§2.7.3) → the split two-button
   *  rendering; `null` when nothing diverted → the single common-root button. */
  readonly divertRootDisplay: string | null;
}

/** The §5.3 OpenActions (slice renderer). [Build-Session-Entscheidung: P3.59] */
export function OpenActions({ commonRootDisplay, divertRootDisplay }: OpenActionsProps) {
  const rootId = useId();
  const divertId = useId();
  // §7.7: fire-and-forget — the shell-out has no UI result to await, and a rejection surfaces via the §7.5.1
  // global frontend-error bridge (the `cancelIntakeCollect` precedent). The buttons stay available (§5.2 row 8).
  const openCommonRoot = useCallback((): void => {
    void openResultTarget("commonRoot");
  }, []);
  const openDivertRoot = useCallback((): void => {
    void openResultTarget("divertRoot");
  }, []);

  const buttonClass = "rounded-md border border-border px-4 py-2 text-base text-text";

  // [Build-Session-Entscheidung: P3.59] §1.12 says the open-folder button is "labelled by `common_root_display`",
  // while §5.3 [DECIDED] pins its LABEL to the exact `strings/ui.ts` entry ("Open folder"/"Open source folder").
  // Both hold: the ui.ts string is the button's visible label + accessible name (§5.6 — the visible label IS the
  // accessible name, WCAG 2.5.3; an `aria-label` of the root would override and break that), and the root display
  // names the DESTINATION in a visible line the button `aria-describedby`s — so a keyboard/SR user hears "Open
  // folder … /src" and no rendering leaves the target unnamed. It mirrors the split connector below, which names
  // the divert root the same way. The string is a lossy display form (§2.10.1), never re-submitted as a path.
  const rootLine = (
    <p id={rootId} className="text-sm text-text-muted">
      {commonRootDisplay}
    </p>
  );

  if (divertRootDisplay === null) {
    return (
      <div className="flex flex-col gap-2">
        {rootLine}
        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={openCommonRoot}
            aria-describedby={rootId}
            className={buttonClass}
          >
            {ui.summary_open_folder}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {/* §5.3: the connector explains WHY there are two buttons — the outputs split across two roots (§2.7.3).
          It also NAMES the divert root, so it is the divert button's `aria-describedby` target — the symmetric
          counterpart of `rootLine` for the common-root button, so neither button leaves its destination unnamed
          to a keyboard/SR user who tabs straight onto it. */}
      <p id={divertId} className="text-sm text-text-muted">
        {formatSavedToConnector(divertRootDisplay)}
      </p>
      {rootLine}
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={openCommonRoot}
          aria-describedby={rootId}
          className={buttonClass}
        >
          {ui.summary_open_source_folder}
        </button>
        <button
          type="button"
          onClick={openDivertRoot}
          aria-describedby={divertId}
          className={buttonClass}
        >
          {ui.summary_open_saved_to_folder}
        </button>
      </div>
    </div>
  );
}
