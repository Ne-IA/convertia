// src/a11y/announcer.ts -- the section 5.6 screen-reader announcement helper.
//
// A single screen-reader announcement channel backed by two visually-hidden ARIA-live regions
// (polite + assertive, section 5.6). announce(message, priority) writes the message into the
// matching region so assistive tech reads it -- the section 5.6 screen-reader path for state-change
// announcements (collected summary, batch milestones, decision states, errors). This module owns
// ONLY the live-region mechanism; the per-component "when to announce" wiring and the section 5.6
// throttling (e.g. coalescing 1000-item progress) are owned by the components (P4/P8).
// [Build-Session-Entscheidung: P1.39]

export type AnnouncePriority = "polite" | "assertive";

const REGION_ID_PREFIX = "convertia-a11y-live";

// Visually hidden but available to assistive tech -- the standard screen-reader-only clip pattern.
// Applied inline (no colour, so out of the design/tokens.css scope) so the announcer is
// self-contained and needs no stylesheet to function.
function applyScreenReaderOnly(el: HTMLElement): void {
  el.style.position = "absolute";
  el.style.width = "1px";
  el.style.height = "1px";
  el.style.margin = "-1px";
  el.style.padding = "0";
  el.style.overflow = "hidden";
  el.style.clip = "rect(0, 0, 0, 0)";
  el.style.whiteSpace = "nowrap";
  el.style.border = "0";
}

function getOrCreateRegion(priority: AnnouncePriority): HTMLElement {
  const id = `${REGION_ID_PREFIX}-${priority}`;
  const existing = document.getElementById(id);
  if (existing !== null) {
    return existing;
  }
  const region = document.createElement("div");
  region.id = id;
  region.setAttribute("aria-live", priority);
  region.setAttribute("aria-atomic", "true");
  region.setAttribute("role", priority === "assertive" ? "alert" : "status");
  applyScreenReaderOnly(region);
  document.body.appendChild(region);
  return region;
}

// Announce `message` to assistive tech via the polite (default) or assertive live region.
export function announce(message: string, priority: AnnouncePriority = "polite"): void {
  getOrCreateRegion(priority).textContent = message;
}
