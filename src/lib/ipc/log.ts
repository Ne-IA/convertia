// src/lib/ipc/log.ts — the §7.5.1 frontend-error → log-file bridge façade (§7.5.3 / §5.1).
//
// The ONE place the WebView imports `@tauri-apps/plugin-log`; `main.tsx` and feature code import THIS
// façade, never `plugin-log` directly — the §5.1 one-IPC-consumer discipline (the `commands.ts` / `events.ts`
// precedent; a G5/eslint `no-restricted-imports` ban on `@tauri-apps/plugin-*` outside `src/lib/ipc/**` joins
// the existing `@tauri-apps/api` ban Co-Pilot-side).
//
// §7.5.1: frontend errors also land in the same local, on-disk log (via `tauri-plugin-log`'s webview→file
// bridge), so a §2.13 app fault in the WebView is recorded without showing the user a stack trace.
//
// §7.5.3-AWARE — path-safe by construction. The bridge records ONLY structural facts: the Error TYPE (a
// built-in Error `name`, or a non-Error value's `typeof`) plus the SOURCE `file:line` (the app's own
// bundled-asset URL, never a user path). It NEVER passes `error.message` / the rejection value / a stack — a
// frontend message can carry a full USER path (§0.11 T2c — the WebView plugin-write surface: `log:default`),
// so there is no level at which a frontend message reaches the log.
//
// [Build-Session-Entscheidung: P2.95] The verbose full-detail level (§7.5.4) is the ENGINE/Rust
// `convertia_core` `debug!` path, gated by P2.94's `log::set_max_level`. The WEBVIEW log target CANNOT reach
// it: `tauri-plugin-log`'s JS log path calls `log::logger().log()` DIRECTLY (its `commands.rs`), bypassing the
// `log::max_level` macro gate — so webview records are filtered only by the plugin's static global `Info`
// level, and a "frontend message only at verbose" is not wireable. It is therefore deliberately NOT attempted;
// a future retrofit that logs the raw message at `debug` would silently re-open the §0.11 T2c path leak (the
// record would surface regardless of verbose). Frontend verbose detail is out of §7.5.4 scope by decision.
import { error as logError, type LogOptions } from "@tauri-apps/plugin-log";

/**
 * [Build-Session-Entscheidung: P2.95] The built-in Error `name`s — the ONLY names passed through verbatim.
 * `Error.prototype.name` is a WRITABLE string, so a crafted / custom `.name` could carry a user path; any name
 * outside this closed set collapses to the generic `"Error"`, keeping the descriptor value-free by
 * construction (§7.5.3 / §0.11 T2c), not merely by convention.
 */
const BUILTIN_ERROR_NAMES: ReadonlySet<string> = new Set([
  "Error",
  "EvalError",
  "RangeError",
  "ReferenceError",
  "SyntaxError",
  "TypeError",
  "URIError",
  "AggregateError",
]);

/**
 * [Build-Session-Entscheidung: P2.95] The structural, path-free descriptor of a caught frontend value: a
 * built-in `Error`'s `name` (`"TypeError"`, `"RangeError"`, …), a generic `"Error"` for a null/absent value OR
 * an Error carrying a non-built-in (crafted) `name`, or — for a non-Error thrown/rejected value — its `typeof`
 * (`"string"` / `"object"` / `"number"` / …). It returns the TYPE, NEVER the value itself, which can carry a
 * user path (§0.11 T2c) — a rejected `"/home/u/secret.jpg"` yields `"string"`, and an Error whose `.name` was
 * set to a path yields `"Error"`, never the path.
 */
export function frontendErrorType(value: unknown): string {
  if (value instanceof Error) {
    return BUILTIN_ERROR_NAMES.has(value.name) ? value.name : "Error";
  }
  if (value === null || value === undefined) {
    return "Error";
  }
  return typeof value;
}

/**
 * [Build-Session-Entscheidung: P2.95] Fire-and-forget log of a structural fact — the `.catch` is load-bearing,
 * not cosmetic: a bare `void logError(...)` that REJECTS would itself become an `unhandledrejection`, which
 * this module's own listener re-consumes → a self-feeding loop (and, outside the Tauri shell, `invoke` always
 * rejects). Swallowing the rejection makes the "never surfaces, never blocks" best-effort contract (§7.4.2 /
 * §2) actually hold.
 */
function safeLog(message: string, options?: LogOptions): void {
  void logError(message, options).catch(() => undefined);
}

/**
 * [Build-Session-Entscheidung: P2.95] Install the §7.5.1 frontend-error → log-file bridge: a `window` `error`
 * + `unhandledrejection` listener that records ONLY the structural facts (§7.5.3) — the Error TYPE + the
 * source `file:line` — via `tauri-plugin-log`'s `error()`. Additive: it does not `preventDefault`, so the dev
 * console still surfaces the error. Called once from `main.tsx`.
 */
export function installFrontendErrorLog(): void {
  window.addEventListener("error", (event: ErrorEvent) => {
    // The TYPE + the source location (the app's own bundled-asset URL:line — never a user path); NEVER
    // `event.message` / `event.error.message` (§0.11 T2c). `event.error` is DOM-typed `any`, narrowed to
    // `unknown` before the structural extraction.
    safeLog(frontendErrorType(event.error as unknown), {
      file: event.filename || undefined,
      line: event.lineno || undefined,
    });
  });
  window.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) => {
    // A rejection carries no source location; log its TYPE only, never `event.reason` (which may be or carry a
    // user path). `event.reason` is DOM-typed `any`, narrowed to `unknown`.
    safeLog(`rejected: ${frontendErrorType(event.reason as unknown)}`);
  });
}
