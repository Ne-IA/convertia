// src/state/store.ts — the §5.1 shared app-store SHELL (Zustand), selector-granular (§1.10).
//
// The single in-memory frontend app store (§5.1 `[DECIDED — Zustand]`). It holds the §5.2
// screen-machine state, the frozen collected batch, the chosen target + options, the resolved
// "will save to …" destination preview, the live per-item progress map, and the §5.8
// `pendingVideoReencodeNote` carry-over field.
//
// SCOPE — P1 lands the typed SHAPE + the selector-granular store hook as scaffolding ONLY.
// The named owning boxes fill the behaviour (this is a sanctioned compile-time interface
// shell, not a quiet gap):
//   - the §5.2 reducer finite-state machine that DRIVES `machine` is `state/machine.ts`
//     (P3.53 slice subset → P4.80 all 12 states); this store only HOLDS the state value.
//   - the live-progress wiring (the §5.8 `Channel<ConversionEvent>` → an `applyConvertEvent`
//     reducer) and the `pendingVideoReencodeNote` population from `RunStarted.willReencode`
//     are filled by P2.120's async model — so this shell carries NO actions yet.
//   - the per-feature shell types below are minimal P1 seams the named boxes expand;
//     §0.6 is the Rust source of truth for the domain model, mirrored to the WebView via the
//     §0.4.5 generated `bindings.ts` (the typed IPC door), which is empty of these DTOs until
//     P2 authors the C-commands. They are deliberately NOT the final domain types.
// Distinct from the Rust-side `tauri-plugin-store` `settings.json` prefs blob (P1.14/P2.85);
// this is the in-memory frontend app store. [Build-Session-Entscheidung: P1.31]
import { create } from "zustand";

import type { ItemId } from "../lib/ipc/commands";

// ─── shell domain types (expanded/replaced by the named owning boxes) ───────────────

/** The §5.2 screen-state value the store holds. The full 12-variant discriminated-union
 *  `State` + reducer is `state/machine.ts` (P3.53 → P4.80); P1 holds only the initial tag. */
export type ScreenState = { readonly tag: "idle" };

/** The frozen collected batch (§1.3 / §0.6 `CollectedSet::Single`); P2 mirrors the real §0.6
 *  wire type via §0.4.5 `bindings.ts`. */
export type CollectedBatch = { readonly setId: string; readonly itemCount: number };

/** The chosen target format + its options (§1.5 / §1.6); P2/P4 fill the real target +
 *  options-registry types. */
export type ChosenTarget = { readonly targetId: string };

/** The resolved "will save to …" destination preview (§1.8 / §2.7, C4 `plan_output`); P2
 *  fills the real `OutputPlanPreview`-derived shape. */
export type DestinationPreview = { readonly willSaveTo: string };

/** One row of the live per-item progress map (§5.8 `ItemProgress` / `ItemFinished`); P2.120
 *  fills the reducer that populates it from the `Channel<ConversionEvent>`. `fraction` is
 *  `null` for an indeterminate stage (§5.8). */
export type ItemProgress = { readonly fraction: number | null; readonly done: boolean };

// ─── the store shape ───────────────────────────────────────────────────────────────────────

export interface AppStore {
  /** Current §5.2 screen state (driven by the P3.53 reducer once it lands). */
  readonly machine: ScreenState;
  /** The frozen collected batch, or `null` before intake. */
  readonly batch: CollectedBatch | null;
  /** The chosen target + options, or `null` before the Targets step. */
  readonly chosenTarget: ChosenTarget | null;
  /** The resolved-destination preview, or `null` before C4 `plan_output` resolves. */
  readonly destination: DestinationPreview | null;
  /** Live per-item progress, keyed by the §0.6 `ItemId`. Consumed via a SELECTOR (§1.10) so a
   *  1000-row virtualised `ProgressList` re-renders per row, not per progress tick. */
  readonly progress: Readonly<Record<ItemId, ItemProgress>>;
  /** §5.8 carry-over: the worst-case `video_reencode` `ConvertingNote` text, set from the C3
   *  `Target.lossy` at the Targets step (4), carried 4→7, then kept or cleared (`null`) by
   *  `RunStarted.willReencode`. `null` when no worst-case re-encode note applies. */
  readonly pendingVideoReencodeNote: string | null;
}

const initialAppState: AppStore = {
  machine: { tag: "idle" },
  batch: null,
  chosenTarget: null,
  destination: null,
  progress: {},
  pendingVideoReencodeNote: null,
};

/** The §5.1 shared app store. Subscribe with a SELECTOR — `useAppStore((s) => s.machine)` —
 *  so a component re-renders only when its selected slice changes (§1.10 selector granularity),
 *  never on every store write. P1 is a read-only shell: the state-mutating actions (the §5.2
 *  reducer dispatch, the §5.8 `applyConvertEvent`) are added by their owning boxes (P3.53 /
 *  P2.120), which expand `AppStore` with their action signatures. */
export const useAppStore = create<AppStore>()(() => ({ ...initialAppState }));
