import { describe, it, expect, afterEach, beforeEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 AppFaultNotice — the §2.13.3 app-level fault screen (state 12). The load-bearing
// leg is the VERBATIM-render contract (the 2026-07-16 P3.60 ruling): the §2.13.3/§7.2-owned `AppFault.message`
// reaches the user unmodified, per kind, with no chrome body of its own — the orphaned-string defect that
// ruling exists to prevent. Plus the trace-free promise, the Start-over exit (the P3 screen-box wiring model),
// and the §5.10:1223/:1245 keyboard contract. [Build-Session-Entscheidung: P3.60]
import { AppFaultNotice } from "./AppFaultNotice";
import { useAppStore } from "../state/store";
import type { AppFault } from "../lib/ipc/commands";

// The §7.2-owned BundleDamaged line — the §2.13.3 "download it again … official releases page" pattern. It is
// the concrete reason state 12 must render `message`: a chrome-only screen would tell this user "the
// conversion stopped" (factually wrong — nothing was converting) and drop the actionable half entirely.
const bundleDamaged: AppFault = {
  kind: "bundleDamaged",
  message:
    "ConvertIA can't start because part of the app appears to be missing or damaged. Try downloading it again from the official releases page.",
};

const engineMissing: AppFault = {
  kind: "engineMissing",
  message: "ConvertIA is missing one of its built-in tools.",
};

// The §5.3:309 `onStartOver` callback is the component's contract (it is presentational — the RerunPrompt
// precedent). App wires it to the machine's `startOver` Msg; these legs wire it to the SAME real store dispatch,
// so the "returns to Idle" assertions exercise the real reducer arm rather than a spy.
const startOver = (): void => {
  useAppStore.getState().dispatch({ type: "startOver" });
};

beforeEach(() => {
  useAppStore.setState({ machine: { tag: "appFault", fault: bundleDamaged } });
});
afterEach(() => {
  cleanup();
  useAppStore.setState({ machine: { tag: "idle" } });
});

describe("AppFaultNotice — §5.2 AppFault (state 12)", () => {
  it("renders the wire AppFault.message VERBATIM (the §2.13.3/§7.2-owned line, P3.60 ruling)", () => {
    const { getByText } = render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    expect(getByText(bundleDamaged.message)).not.toBeNull();
  });

  it("renders a DIFFERENT kind's message verbatim too — no per-kind chrome branch (one string, one home)", () => {
    // §2.8.2 deliberately homes NO row for the three app-level kinds ("render via the §2.13.3 app://fault
    // catalog"), so the screen must pass whatever the core resolved straight through.
    const { getByText } = render(<AppFaultNotice fault={engineMissing} onStartOver={startOver} />);
    expect(getByText(engineMissing.message)).not.toBeNull();
  });

  it("does NOT paraphrase or substitute the run-path chrome literal for a DTO-carrying fault", () => {
    // The §5.8 run-path line ("…the conversion stopped") belongs to P4.50's DTO-less class; rendering it here
    // would drop the §7.2 copy — the exact orphaned-string failure the P3.60 ruling rejected as option B.
    const { queryByText } = render(
      <AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />,
    );
    expect(queryByText(/the conversion stopped/)).toBeNull();
  });

  it("shows a calm chrome heading beside the verbatim body (§5.7: the UI owns the frame, §02/§7.2 the words)", () => {
    const { getByRole } = render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    const heading = getByRole("heading", { name: "Something went wrong" });
    expect(heading.getAttribute("aria-live")).toBe("assertive");
  });

  it("renders NO stack trace — the body is exactly the core's calm line (§2.13/§2.13.3)", () => {
    const withTrace: AppFault = { kind: "webviewFault", message: "Something went wrong." };
    const { container } = render(<AppFaultNotice fault={withTrace} onStartOver={startOver} />);
    const text = container.textContent ?? "";
    // The rendered surface is heading + message + button — nothing else leaks in.
    expect(text).toBe("Something went wrong" + withTrace.message + "Start over");
  });

  it("focuses Start over on entry — state 12's only action, so Enter activates it (§5.10:1245)", () => {
    const { getByRole } = render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    expect(document.activeElement).toBe(getByRole("button", { name: "Start over" }));
  });

  it("Start over dispatches startOver → the machine returns to Idle (§5.2 row 12)", () => {
    const { getByRole } = render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    fireEvent.click(getByRole("button", { name: "Start over" }));
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("Esc → Start over (§5.10:1245 'identical; no other choice')", () => {
    render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("Ctrl/⌘+N → Start over (the §5.10:1223 chord, bound in AppFault)", () => {
    render(<AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />);
    fireEvent.keyDown(document, { key: "n", ctrlKey: true });
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("is NOT a modal — a full-screen state (§5.7:840)", () => {
    const { queryByRole } = render(
      <AppFaultNotice fault={bundleDamaged} onStartOver={startOver} />,
    );
    expect(queryByRole("alertdialog")).toBeNull();
    expect(queryByRole("dialog")).toBeNull();
  });
});
