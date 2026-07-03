// src/lib/ipc/commands.ts — the §5.1 typed COMMAND façade (§0.4 / §0.4.5).
//
// Re-exports the tauri-specta-generated command wrappers + DTO types from the single generated
// `bindings.ts` (§0.4.5), so feature code imports its typed command-calling surface from HERE and never
// touches raw `@tauri-apps/api` invoke — the §5.1 one-IPC-consumer discipline (only `src/lib/ipc/**`
// imports the Tauri IPC surface: `@tauri-apps/api` + any `@tauri-apps/plugin-*` package), enforced by the
// P1.36/G5 ESLint rule from the first commit. The C1..C13 wrappers are an EMPTY generated set in P1: `bindings.ts` currently exposes only
// the §0.6 identity types, so this re-export surfaces those today and picks up the generated `commands`
// object automatically when P2 authors the `#[tauri::command]` handlers — with no edit here.
//
// `export *` (NOT `export { commands }`, and NOT `export type *`) is the deliberate, forward-compatible
// form: a named `export { commands }` would not type-check while the generated set is empty; and
// `export type *` re-exports ONLY types — it would SILENTLY DROP the generated `commands` VALUE object
// P2 emits (TS semantics: `export type *` prevents value propagation). `export *` is always retained
// (TS 3.9+), re-exports values AND types, and is legal over a currently types-only module under
// `isolatedModules` / `verbatimModuleSyntax` — so it surfaces P2's `commands` value with no edit here.
// Do NOT "simplify" this to `export type *`. [Build-Session-Entscheidung: P1.27]
export * from "./bindings";
