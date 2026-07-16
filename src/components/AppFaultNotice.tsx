// src/components/AppFaultNotice.tsx — the §5.3 AppFaultNotice: the app-level fault screen (state 12) (P3.60).
//
// The §2.13.3 calm single screen: a plain, TRACE-FREE line + **Start over** → `Idle`. It never fabricates
// per-item outcomes for items it never heard back about (§5.2 row 12 / §5.8).
//
// THE BODY IS THE WIRE `AppFault.message`, RENDERED VERBATIM — the load-bearing contract of this component
// (the 2026-07-16 P3.60 ruling, option A). §7.2/§2.13.3 own those WORDS: `message` is the "§2.13.3
// pre-localised, plain-English, trace-free calm message" (§0.4.3), and `crate::outcome` deliberately refuses
// §2.8.2 homing for the three app-level kinds ({`EngineMissing`, `WebviewFault`, `BundleDamaged`}) precisely
// because they "render via the §2.13.3 `app://fault` catalog" — one string, one home. So this component
// authors NO body copy: a chrome line here would leave the §7.2 strings with no renderer anywhere (the
// orphaned-string class) and would show a damaged-bundle user the factually wrong "the conversion stopped".
// Only the heading + the Start-over label are chrome (§5.7:799: §02/§7.2 own the words, the UI owns the frame).
//
// NO kind-switch, NO fallback line: the machine's state-12 payload is a non-null `AppFault` (P3.53), and in P3
// every entry into state 12 carries a real DTO — the `app://fault` wildcard is the only runtime entry (its
// production EMIT is itself the P4 readiness/`PendingFault` body, `main.rs present_startup_fault`). The DTO-less
// RUN-PATH fault (§5.8's Channel-silence / opaque C6-C7 reject, whose core is dead and can author nothing) is
// **P4.50's** leg: it owns supplying `onRunFault`, the channel-silence watchdog, the machine re-cut that carries
// a DTO-less fault, and that class's chrome copy. This box never synthesizes a wire `AppFault` client-side
// (§5.2: the backend is the source of truth for facts).
//
// SLICE SCOPE (P3.60): focus-on-entry lands on Start over — the screen's ONLY action — so §5.10:1245's
// "Enter → Start over" is native <button> activation and Esc maps to the same single action. The §5.6.1(2)
// assertive announce-ON-ENTRY (distinct from the §5.6.1(1) `aria-live` heading attribute this box builds) is
// **P4.75**'s, which fires the shared `announcer.ts` live region per state transition and names state 12
// explicitly — the P3.59 precedent deferred its own assertive outcome announcement the same way. Visual polish
// is P8; the chrome-copy refinement is P8.19.1; P4.69's error/edge-state framework supersedes this slice
// screen, carrying the verbatim-`message` model forward unchanged. [Build-Session-Entscheidung: P3.60]
import { useEffect, useRef } from "react";

import { keymap, matchesAccelerator } from "../a11y/keymap";
import type { AppFault } from "../lib/ipc/commands";
import { ui } from "../strings/ui";

export interface AppFaultNoticeProps {
  /** The §2.13 app-level fault from the machine's state-12 payload. Its `message` is rendered VERBATIM (see the
   *  module header); its `kind` is NOT switched on — the core already resolved the kind to its calm line, so a
   *  per-kind branch here would re-implement the §2.13.3 catalog it deliberately does not home in §2.8.2. */
  readonly fault: AppFault;
  /** §5.2 row 12: the single Start-over action (button, Esc, or the §5.10 Ctrl/⌘+N chord) → `Idle`. A CALLBACK,
   *  not an internal dispatch, per the §5.3:309 prop contract — this component is presentational (the
   *  `RerunPrompt` precedent, whose §5.3 row likewise lists its callbacks). Its two sibling state screens take
   *  no callback because their §5.3 rows declare none. */
  readonly onStartOver: () => void;
}

/** The §5.3 AppFaultNotice (§5.2 state 12). [Build-Session-Entscheidung: P3.60] */
export function AppFaultNotice({ fault, onStartOver }: AppFaultNoticeProps) {
  const startOverRef = useRef<HTMLButtonElement>(null);

  // §5.10:1245 — Start over is state 12's ONLY action, so focus lands on it and Enter activates it natively.
  useEffect(() => {
    startOverRef.current?.focus();
  }, []);

  // §5.10:1223/:1245 — the Ctrl/⌘+N "Start over" chord (bound in AppFault per the canonical table) and Esc,
  // which §5.10:1245 maps to the SAME single action ("identical; no other choice").
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape" || matchesAccelerator(event, keymap.startOver)) {
        event.preventDefault();
        onStartOver();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [onStartOver]);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      {/* Announced assertively on entry — the app-level fault is something the user must notice (§5.6). */}
      <h2 aria-live="assertive" className="text-xl font-semibold text-text">
        {ui.appfault_heading}
      </h2>
      {/* The §2.13.3/§7.2-owned calm line, VERBATIM — no stack trace, no chrome path line, no paraphrase. */}
      <p className="text-base text-text">{fault.message}</p>
      <button
        ref={startOverRef}
        type="button"
        onClick={onStartOver}
        className="self-start rounded-md bg-accent px-4 py-2 text-base font-medium text-accent-contrast"
      >
        {ui.appfault_start_over}
      </button>
    </div>
  );
}
