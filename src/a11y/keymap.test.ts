import { describe, it, expect, vi, afterEach } from "vitest";

import { keymap, matchesAccelerator, type Accelerator } from "./keymap";

// Section 5.10: matchesAccelerator resolves a chord against a KeyboardEvent. (jsdom reports a
// non-macOS platform, so these exercise the Ctrl branch of CmdOrCtrl.)
describe("keymap (section 5.10 accelerator table)", () => {
  it("matches a chord when its CmdOrCtrl modifier is held", () => {
    const event = new KeyboardEvent("keydown", { key: "o", ctrlKey: true });
    expect(matchesAccelerator(event, keymap.openFilePicker)).toBe(true);
  });

  it("does not match the same key without the CmdOrCtrl modifier", () => {
    const event = new KeyboardEvent("keydown", { key: "o" });
    expect(matchesAccelerator(event, keymap.openFilePicker)).toBe(false);
  });

  it("distinguishes a Shift chord from its non-Shift sibling on the same key", () => {
    const event = new KeyboardEvent("keydown", { key: "o", ctrlKey: true, shiftKey: true });
    expect(matchesAccelerator(event, keymap.chooseFolder)).toBe(true);
    expect(matchesAccelerator(event, keymap.openFilePicker)).toBe(false);
  });

  it("rejects a chord when a stray Alt is held", () => {
    const event = new KeyboardEvent("keydown", { key: "n", ctrlKey: true, altKey: true });
    expect(matchesAccelerator(event, keymap.startOver)).toBe(false);
  });

  it("matches both About accelerators: F1 and the ? (Shift+/) help-key (section 5.10 'F1 / ?')", () => {
    expect(matchesAccelerator(new KeyboardEvent("keydown", { key: "F1" }), keymap.about)).toBe(
      true,
    );
    expect(
      matchesAccelerator(
        new KeyboardEvent("keydown", { key: "?", shiftKey: true }),
        keymap.aboutAlt,
      ),
    ).toBe(true);
    // a plain "/" without Shift is not the ? help-key
    expect(matchesAccelerator(new KeyboardEvent("keydown", { key: "/" }), keymap.aboutAlt)).toBe(
      false,
    );
  });

  // P2.137: whole-table chord uniqueness — section 5.10's "single source" table must never bind two actions
  // to the same chord. Uniqueness is checked at the MATCHER's granularity: `key` matches case-insensitively
  // and the two modifier flags default to false, so the key+cmdOrCtrl+shift triple is exactly what
  // `matchesAccelerator` discriminates on.
  it("binds every action to a unique key+cmdOrCtrl+shift chord (whole-table uniqueness)", () => {
    const chords: readonly Accelerator[] = Object.values(keymap);
    const triples = chords.map(
      (chord) =>
        `${chord.key.toLowerCase()}|${String(chord.cmdOrCtrl ?? false)}|${String(chord.shift ?? false)}`,
    );
    expect(new Set(triples).size).toBe(triples.length);
  });
});

// P2.137: the macOS leg of CmdOrCtrl (section 5.10 — Cmd/metaKey on macOS, Ctrl elsewhere). `isMac` is
// resolved ONCE at module load from `navigator.platform`, so the mac branch needs a fresh module instance
// evaluated under a shadowed platform: define an own configurable `platform` on the navigator instance,
// reset the module registry, and dynamically re-import — the static import above keeps the non-mac instance
// for the rest of the file.
describe("keymap macOS leg (CmdOrCtrl resolves to Cmd/metaKey)", () => {
  afterEach(() => {
    // Drop the own-property shadow so the prototype getter (jsdom's non-macOS default) is restored, then
    // reset the registry so no other dynamic import inherits the mac-flavoured module instance.
    Reflect.deleteProperty(navigator, "platform");
    vi.resetModules();
  });

  it("matches Cmd+O (metaKey) and REJECTS Ctrl+O (the wrong primary) on macOS", async () => {
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    vi.resetModules();
    const mac = await import("./keymap");
    expect(
      mac.matchesAccelerator(
        new KeyboardEvent("keydown", { key: "o", metaKey: true }),
        mac.keymap.openFilePicker,
      ),
    ).toBe(true);
    // On macOS the PRIMARY is Cmd; a held Ctrl is the wrong primary and must be rejected, not tolerated.
    expect(
      mac.matchesAccelerator(
        new KeyboardEvent("keydown", { key: "o", ctrlKey: true }),
        mac.keymap.openFilePicker,
      ),
    ).toBe(false);
  });
});
