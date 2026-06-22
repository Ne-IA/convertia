// src/lib/ipc/events.ts — the §5.8 Channel + event-subscription helper home (§0.4.2 / §5.4).
//
// The SINGLE place the WebView wires `@tauri-apps/api` Channel / window-event APIs — the §5.1 hard rule
// "only `src/lib/ipc/**` imports `@tauri-apps/api`", the one-IPC-consumer discipline the P1.36 ESLint
// rule enforces from the first commit. It is the named home for the hand-written subscription helpers
// authored as P2 lands the §0.4.2 event contract + the §1.1 intake flow: the §5.4 native
// `onDragDropEvent` intake wiring, the §5.8 `start_conversion` progress `Channel<ConversionEvent>`
// lifecycle, and the §0.4.1 C1/C2a `onScan` `Channel<ScanProgress>` telemetry.
//
// P1 establishes the named seam only. Unlike `commands.ts`, there is nothing to re-export here: the
// event helpers are HAND-WRITTEN (not tauri-specta output), and `collect_events![]` is the empty set
// until P2, so `bindings.ts` exposes no generated event surface. The empty `export {}` keeps this a
// module (so the P1.36 import-boundary lint and a future `@tauri-apps/api` import have a real home)
// without inventing an unbacked helper. [Build-Session-Entscheidung: P1.27]
export {};
