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
//
// [Build-Session-Entscheidung: P3.60] **This factory must export EVERY events-façade binding `App.tsx` imports —
// including module-SCOPE ones.** A `vi.mock` factory is strict: a missing export throws at import time
// ("No <x> export is defined on the … mock") and the whole file fails to COLLECT, so the miss reds this G33a leg
// (and the coverage leg) while `pnpm test` stays green — the a11y legs are `exclude`d from `vitest.config.ts` by
// design, so they cannot catch each other. P3.60's `APP_EVENT_HANDLERS` (App.tsx, module scope) made
// `consumeAppFault` the first such import and tripped exactly this. **The class:** a new façade import in
// `App.tsx` must be added to BOTH App mock factories (here + `App.test.tsx`) in the SAME commit — and a screen
// box must run `pnpm test:a11y`, not only `pnpm test`. (The sibling class: the P1.35/ee362ce mount-side-effect
// note — an IPC side effect at mount breaks a11y AND coverage; the fix is mocking the FAÇADE, isolation not
// suppression.)
vi.mock("./lib/ipc/events", () => ({
  // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] the mount handshake now calls the consuming
  // `consumeMountDrain` (not the bare `drainPendingIntake`); the a11y baseline is unchanged (Idle renders the
  // DropZone). The advanceToTargets/cancelIntakeCollect stubs feed the statically-imported Confirm/Collecting
  // router arms P3.55 added (unused in the Idle render this baseline exercises).
  consumeMountDrain: () => Promise.resolve(),
  subscribeAppEvents: () => Promise.resolve(() => {}),
  subscribeNativeDragDrop: () => Promise.resolve(() => {}),
  // The §5.2 Idle screen (the P3.54 DropZone, rendered by App) imports the C2a `pickForIntake` façade — stub it
  // so this a11y baseline render stays hermetic. The DropZone's own axe legs live in DropZone.a11y.test.tsx.
  pickForIntake: () => Promise.resolve(),
  advanceToTargets: () => Promise.resolve(),
  cancelIntakeCollect: () => Promise.resolve(),
  // The §5.8 `app://fault` consumption App wires into its module-scope handler set (P3.60) — see the note above.
  consumeAppFault: () => undefined,
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
