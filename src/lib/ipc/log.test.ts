import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";

// §6.4.6 unit (G15): the §7.5.1 frontend-error → log-file bridge (P2.95). Mock `@tauri-apps/plugin-log`'s
// `error` so the bridge runs with no Tauri runtime and we read back the EXACT (message, options) it sends —
// the load-bearing assertion being §7.5.3 / §0.11 T2c: a user path in a frontend error message / rejection
// value / crafted Error name is NEVER in the log output. [Build-Session-Entscheidung: P2.95]
const logError = vi.fn<(message: string, options?: unknown) => Promise<void>>();
vi.mock("@tauri-apps/plugin-log", () => ({
  error: (message: string, options?: unknown) => logError(message, options),
}));

import { frontendErrorType, installFrontendErrorLog } from "./log";

// A realistic user path a frontend Error message / rejection value could carry (§0.11 T2c) — the exact thing
// that must NEVER reach the log.
const SECRET_PATH = "/home/alice/secret-project/vacation.jpg";

// Every argument of every logged call, serialized — the one invariant is that a user path is absent from ALL
// of it (message AND options), on every call.
function loggedText(): string {
  return logError.mock.calls
    .flat()
    .map((arg) => (typeof arg === "string" ? arg : (JSON.stringify(arg) ?? "")))
    .join(" ");
}

describe("frontendErrorType (§7.5.3 structural type, never the value)", () => {
  it("returns a built-in Error's name", () => {
    expect(frontendErrorType(new TypeError("boom"))).toBe("TypeError");
    expect(frontendErrorType(new RangeError("boom"))).toBe("RangeError");
    expect(frontendErrorType(new Error("boom"))).toBe("Error");
  });

  it('collapses a crafted (non-built-in) Error name to "Error", never logging it (§0.11 T2c)', () => {
    // `Error.prototype.name` is writable — a crafted name that IS a user path must NOT pass through.
    const crafted = new Error("boom");
    crafted.name = SECRET_PATH;
    expect(frontendErrorType(crafted)).toBe("Error");
    expect(frontendErrorType(crafted)).not.toContain("alice");
  });

  it("returns the TYPE, never the VALUE, for a non-Error rejection (§0.11 T2c)", () => {
    // The critical case: a string reason that IS a user path → "string", never the path.
    expect(frontendErrorType(SECRET_PATH)).toBe("string");
    expect(frontendErrorType(SECRET_PATH)).not.toContain("alice");
    expect(frontendErrorType({ path: SECRET_PATH })).toBe("object");
    expect(frontendErrorType(42)).toBe("number");
  });

  it('returns a generic "Error" for a null/undefined value (never "object" for null)', () => {
    expect(frontendErrorType(null)).toBe("Error");
    expect(frontendErrorType(undefined)).toBe("Error");
  });
});

describe("installFrontendErrorLog (§7.5.1 bridge — structural facts only)", () => {
  beforeAll(() => {
    // Install ONCE — a single window "error"/"unhandledrejection" listener for the file; each test dispatches
    // and resets the mock, so listeners never accumulate into duplicate log calls.
    installFrontendErrorLog();
  });
  beforeEach(() => {
    logError.mockReset();
    logError.mockResolvedValue(undefined);
  });

  it("logs the error TYPE + source, never the message path, on a window error (§0.11 T2c)", () => {
    window.dispatchEvent(
      new ErrorEvent("error", {
        error: new TypeError(`failed reading ${SECRET_PATH}`),
        message: `failed reading ${SECRET_PATH}`,
        filename: "http://tauri.localhost/assets/index.js",
        lineno: 42,
      }),
    );
    expect(logError).toHaveBeenCalledTimes(1);
    // Structural type only + the source location (the app's own asset URL:line, never a user path).
    expect(logError).toHaveBeenCalledWith("TypeError", {
      file: "http://tauri.localhost/assets/index.js",
      line: 42,
    });
    // The user path is absent from every logged argument (message AND options).
    expect(loggedText()).not.toContain(SECRET_PATH);
    expect(loggedText()).not.toContain("alice");
  });

  it("logs a generic Error without the message even when no error object is present", () => {
    // A bare-message error (event.error null) must STILL never surface event.message (which carries the path).
    window.dispatchEvent(
      new ErrorEvent("error", {
        error: null,
        message: `failed reading ${SECRET_PATH}`,
        filename: "http://tauri.localhost/assets/index.js",
        lineno: 7,
      }),
    );
    expect(logError).toHaveBeenCalledTimes(1);
    expect(logError).toHaveBeenCalledWith("Error", {
      file: "http://tauri.localhost/assets/index.js",
      line: 7,
    });
    expect(loggedText()).not.toContain("alice");
  });

  it("logs a rejection's TYPE, never the reason value, on an unhandled rejection (§0.11 T2c)", () => {
    // A rejection whose reason IS a user-path string must log only its TYPE ("string"), never the path.
    const rejected = Promise.reject(SECRET_PATH);
    // Keep the test's OWN promise from becoming a real unhandled rejection in the runner.
    void rejected.catch(() => undefined);
    window.dispatchEvent(
      new PromiseRejectionEvent("unhandledrejection", {
        promise: rejected,
        reason: SECRET_PATH,
      }),
    );
    expect(logError).toHaveBeenCalledTimes(1);
    expect(logError).toHaveBeenCalledWith("rejected: string", undefined);
    expect(loggedText()).not.toContain(SECRET_PATH);
    expect(loggedText()).not.toContain("alice");
  });
});
