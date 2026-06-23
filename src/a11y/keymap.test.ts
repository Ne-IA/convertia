import { describe, it, expect } from "vitest";

import { keymap, matchesAccelerator } from "./keymap";

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
});
