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
//   - the live-progress wiring — the §5.8 `Channel<ConversionEvent>` → the `applyConvertEvent`
//     reducer (the per-item `progress` map) + the `pendingVideoReencodeNote` keep/clear from
//     `RunStarted.willReencode` — is LANDED by P2.120 (the store's first action, the pure
//     `reduceConvertEvent` below); the §5.2 `machine` reducer dispatch stays P3.53's.
//   - the per-feature shell types below are minimal P1 seams the named boxes expand;
//     §0.6 is the Rust source of truth for the domain model, mirrored to the WebView via the
//     §0.4.5 generated `bindings.ts` (the typed IPC door), which is empty of these DTOs until
//     P2 authors the C-commands. They are deliberately NOT the final domain types.
// Distinct from the Rust-side `tauri-plugin-store` `settings.json` prefs blob (P1.14/P2.85);
// this is the in-memory frontend app store. [Build-Session-Entscheidung: P1.31]
import { create } from "zustand";

import type { ConversionEvent, EngineHealth, ItemId, TargetId } from "../lib/ipc/commands";

import { initialState, transition, type Msg, type State } from "./machine";

// The §5.2 screen-state machine now lives in `state/machine.ts` (the P3.53 slice-subset `State` +
// pure `transition` reducer, the 2026-07-13 P3.53 ruling); the store HOLDS `machine: State` and drives it
// via {@link AppStore.dispatch}. Re-exported here so the §5.3 screens read/dispatch from the one store home.
export type { Msg, State } from "./machine";

// ─── shell domain types (expanded/replaced by the named owning boxes) ───────────────

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

/** The DATA slice of the §5.1 store (state only). Split from the full {@link AppStore} so the pure
 *  `reduceConvertEvent` can take/return it without depending on the actions. */
export interface AppState {
  /** Current §5.2 screen state — the P3.53 `state/machine.ts` slice-subset {@link State}, driven by
   *  {@link AppStore.dispatch} through the pure `transition` reducer (§5.2). P4.80 adds the remaining states. */
  readonly machine: State;
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
  /** The cached C12 `EngineHealth` (§7.2.3), or `null` before the §7.2.3 startup probe (P4.45) has
   *  populated it. The FULL health object is held — not only the target-id set — so P4.70.2's
   *  disable-with-REASON FormatPicker tiles derive BOTH the unavailable set AND its per-target reason
   *  from this one field. Read through `selectUnavailableTargets` (§1.10 selector granularity).
   *  [Build-Session-Entscheidung: P2.114] */
  readonly engineHealth: EngineHealth | null;
}

/** The §5.1 shared app store = the {@link AppState} data + the state-mutating actions its owning boxes add.
 *  P1 held only data; P2.120 lands the first action (`applyConvertEvent`); the §5.2 `machine` reducer dispatch
 *  is added by P3.53. Subscribe with a SELECTOR (§1.10). */
export interface AppStore extends AppState {
  /** [P2.120] Reduce one §0.4.2 `ConversionEvent` — the §5.8 `start_conversion` `Channel<ConversionEvent>`
   *  stream — into the store: the per-item `progress` map from `itemStarted`/`itemProgress`/`itemFinished`,
   *  and the §5.8 keep/clear of `pendingVideoReencodeNote` on `runStarted.willReencode`. It writes NO other
   *  field (the §1.11 aggregate batch bar + the §1.12 terminal `RunResult` are the P3.53 machine's, §5.8). */
  readonly applyConvertEvent: (event: ConversionEvent) => void;
  /** [P3.53] Dispatch a §5.2 machine `Msg` — apply the pure `transition` reducer to advance `machine` (§5.2).
   *  The §5.3 screens dispatch these (user actions + inbound §5.8 IPC results/events); the machine is the flow
   *  single-source-of-truth (the 2026-07-13 P3.53 ruling), so the screens hold NO transition logic. */
  readonly dispatch: (msg: Msg) => void;
}

const initialAppState: AppState = {
  machine: initialState(),
  batch: null,
  chosenTarget: null,
  destination: null,
  progress: {},
  pendingVideoReencodeNote: null,
  engineHealth: null,
};

/** The §5.1 shared app store. Subscribe with a SELECTOR — `useAppStore((s) => s.machine)` — so a component
 *  re-renders only when its selected slice changes (§1.10 selector granularity), never on every store write.
 *  P2.120 adds the first action, `applyConvertEvent` (a thin `set` over the pure `reduceConvertEvent`); the
 *  §5.2 `machine` reducer dispatch is added by P3.53. */
export const useAppStore = create<AppStore>()((set) => ({
  ...initialAppState,
  applyConvertEvent: (event) => set((state) => reduceConvertEvent(state, event)),
  dispatch: (msg) => set((state) => ({ machine: transition(state.machine, msg) })),
}));

/** The pure §5.8 progress reducer behind `applyConvertEvent` — `(state, event) → changed slice` so it is
 *  unit-testable without a live store (zustand `set` merges the returned partial). Exhaustive over the six
 *  §0.4.2 `ConversionEvent` variants; the two the P2 store holds no field for (`batchProgress`, `runFinished`)
 *  return an empty slice with their real consumer named. [Build-Session-Entscheidung: P2.120] */
export function reduceConvertEvent(state: AppState, event: ConversionEvent): Partial<AppState> {
  switch (event.type) {
    case "runStarted":
      // §5.8: `willReencode` KEEPS the step-4 `pendingVideoReencodeNote` (whose text P4.65 sets) or CLEARS it
      // when the run took the lossless remux path. The per-run `progress` reset on Converting-entry is the
      // P3.53 machine's job, not this reducer.
      return event.data.willReencode ? {} : { pendingVideoReencodeNote: null };
    case "itemStarted":
      // §1.9 Pending→Running: a determinate row, no fraction reported.
      return {
        progress: { ...state.progress, [event.data.itemId]: { fraction: null, done: false } },
      };
    case "itemProgress":
      return {
        progress: {
          ...state.progress,
          [event.data.itemId]: { fraction: event.data.fraction, done: false },
        },
      };
    case "itemFinished":
      // §1.9 terminal per item: the row is done. The OUTCOME (Succeeded/Failed/Skipped/Cancelled) is surfaced
      // by the §1.12 Summary/RunResult (P3.53 machine), not this minimal live bar.
      return { progress: { ...state.progress, [event.data.itemId]: { fraction: 1, done: true } } };
    case "batchProgress":
      // §1.11 aggregate batch bar — no §5.1 store field; its render is the P3.53 ProgressList's. No store effect.
      return {};
    case "runFinished":
      // §1.12 terminal RunResult → Summary (state 8) is the P3.53 machine + the C8 re-fetch. No §5.1 store field.
      return {};
    default:
      return assertNever(event);
  }
}

/** Exhaustiveness guard: a new `ConversionEvent` variant reaching here fails to compile (`event: never`), so
 *  `reduceConvertEvent` can never silently drop an event. Unreachable by construction. */
function assertNever(event: never): never {
  throw new Error(`unhandled ConversionEvent variant: ${String(event)}`);
}

/** A module-level stable-empty sentinel: the `engineHealth === null` branch of
 *  `selectUnavailableTargets` MUST return a referentially-stable `[]`. A fresh `[]` literal per call
 *  defeats zustand's `Object.is` slice-equality, so a `useAppStore(selectUnavailableTargets)` subscriber
 *  would re-render on EVERY unrelated store write (e.g. the §5.8 progress ticks) across the whole
 *  `engineHealth === null` pre-probe window — breaking the §1.10 selector-granularity guarantee this seam
 *  exists to uphold. [Build-Session-Entscheidung: P2.114] */
const NO_UNAVAILABLE_TARGETS: TargetId[] = [];

/** §5.2 read-seam (§1.10 selector) — the §3.4 patent-gapped / platform-unavailable target set the
 *  FormatPicker disables or omits, read from the cached C12 `EngineHealth.unavailableTargets`
 *  (§7.2.3). Returns the stable-empty `NO_UNAVAILABLE_TARGETS` while `engineHealth` is `null` (the cache
 *  carries no backing data until the §7.2.3 startup probe, P4.45, populates the store) — so the shape is
 *  live from P2 while the data arrives with the P4 probe, and `useAppStore(selectUnavailableTargets)` stays
 *  referentially stable across unrelated writes. This is the typed read-seam P4.70.2's disable-with-reason
 *  tiles bind against. [Build-Session-Entscheidung: P2.114] */
export const selectUnavailableTargets = (s: AppStore): TargetId[] =>
  s.engineHealth?.unavailableTargets ?? NO_UNAVAILABLE_TARGETS;
