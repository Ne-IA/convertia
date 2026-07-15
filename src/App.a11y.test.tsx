import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §5.5 app-chrome a11y BASELINE — the per-push G33a (§6.4.6a) vitest-axe target over the mounted shell.
//
// [Build-Session-Entscheidung: P2.122] SCOPE. The persistent app-chrome frame's VISIBLE content — the
// `AppHeader` banner (BrandLogo + ThemeToggle + About, §5.5) and the twelve §5.2 state screens — is owned by
// P3-P8 (P8.1-P8.3 build the header banner). So P2 (app-shell-contracts) establishes only the a11y baseline
// the shell exposes now and every subsequent screen inherits: the single `<main>` workspace landmark the
// state screens render into, zero axe ARIA/role violations, and roving-tabindex sanity. The banner landmark
// joins this baseline when P8.1 builds the `AppHeader`; each state screen adds its own axe assertions as it
// lands (the App.test.tsx / per-screen split). Rendering an empty banner region here would be premature P8
// chrome with an unfilled landmark, so the shell DOM is unchanged and this file is the enforced baseline
// contract (P1.35 wired the runner + P1.62.5 the armed canary; this box establishes the baseline they scan).
//
// App fires three §5.4/§5.8 IPC mount effects (`useAppEvents`/`useNativeDragDrop`/`useLaunchDrain`); mock the
// §5.8 IPC façade so each a11y render stays hermetic — jsdom has no Tauri runtime, so the real
// Channel/invoke/listen/onDragDropEvent throws. Each helper's behaviour is covered in `lib/ipc/events.test.ts`.
// The vitest-axe `toHaveNoViolations` matcher is not used — its 0.1.0 `.d.ts` re-exports the matcher type-only,
// which `verbatimModuleSyntax` rejects — so the assertions read the `axe()` result directly.
vi.mock("./lib/ipc/events", () => ({
  drainPendingIntake: () => Promise.resolve({ empty: { skipped: [] } }),
  subscribeAppEvents: () => Promise.resolve(() => {}),
  subscribeNativeDragDrop: () => Promise.resolve(() => {}),
  // The §5.2 Idle screen (the P3.54 DropZone, rendered by App) imports the C2a `pickForIntake` façade — stub it
  // so this a11y baseline render stays hermetic. The DropZone's own axe legs live in DropZone.a11y.test.tsx.
  pickForIntake: () => Promise.resolve(),
}));

import { App } from "./App";

// Unmount between legs. Each `it` renders its own `<App/>`, and vitest.a11y.config.ts does not set
// `globals: true`, so @testing-library's auto-cleanup never registers — without this an un-unmounted tree per
// leg accumulates in `document.body`. The three legs below stay correct regardless (they query the per-render
// `container`), but this file is the template the per-screen a11y legs copy, so pinning cleanup here keeps a
// sibling that reaches for `screen.*`/document-scoped queries from tripping over an accumulated DOM.
afterEach(cleanup);

// Three independently-faileable legs so an ARIA/role, landmark, or focus-order regression is its OWN G33a red
// (the dedicated `pnpm test:a11y` leg's purpose, vitest.a11y.config.ts) rather than one merged smoke assertion.
describe("App — §5.5 app-chrome a11y baseline (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(<App />);
    const results = await axe(container);
    // Mapped to rule ids so a failure names the violated axe rule, not an opaque object dump.
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("exposes exactly one `main` workspace landmark (the persistent §5.5 chrome baseline)", () => {
    const { container } = render(<App />);
    // The §5.2 state screens render INTO this single workspace landmark; the baseline is one, never zero
    // (no main = an unlandmarked page) and never two (a duplicate main is an axe best-practice fault).
    expect(container.querySelectorAll("main")).toHaveLength(1);
  });

  it("introduces no positive `tabindex` (roving-tabindex sanity, G33a)", () => {
    const { container } = render(<App />);
    // A `tabindex > 0` overrides natural DOM focus order — an a11y anti-pattern the shell must never seed;
    // the baseline holds it at zero elements, so a screen that subsequently adds one reddens this per-push leg.
    const positiveTabindex = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positiveTabindex).toEqual([]);
  });
});
