import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 DropZone behaviour — the §5.4 DOM drag affordance + the §0.4.1 C2a intake
// wiring + the §5.10 Idle accelerators. Mock the §5.1 IPC façade so the C2a picker fire is observable with no
// Tauri runtime (the DropZone imports `pickForIntake` from src/lib/ipc/events; jsdom has no invoke — the
// P1.35/ee362ce mount-side-effect note). The `pickForIntake` command-arg contract is `events.test.ts`; this
// pins the DropZone's user-action → picker-fire wiring. [Build-Session-Entscheidung: P3.54]
const pickForIntake = vi.fn<(kind: string) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  pickForIntake: (kind: string) => pickForIntake(kind),
}));

import { DropZone } from "./DropZone";
import { ui } from "../strings/ui";

// vitest.config.ts does not set globals:true, so @testing-library's auto-cleanup never registers — unmount
// each render so the §5.10 accelerator effect's document `keydown` listener does not leak across legs.
afterEach(cleanup);

const zoneOf = (container: HTMLElement): HTMLElement => {
  const zone = container.querySelector<HTMLElement>("[data-drag-active]");
  if (zone === null) {
    throw new Error("DropZone root ([data-drag-active]) not found");
  }
  return zone;
};

describe("DropZone — §0.4.1 C2a intake wiring (click / choose-folder)", () => {
  beforeEach(() => {
    pickForIntake.mockReset();
    pickForIntake.mockResolvedValue(undefined);
  });

  it("fires C2a pick_for_intake { kind: files } on the browse-surface click (§5.3)", () => {
    const { getByRole } = render(<DropZone />);
    fireEvent.click(getByRole("button", { name: ui.dropzone_prompt }));
    expect(pickForIntake).toHaveBeenCalledTimes(1);
    expect(pickForIntake).toHaveBeenCalledWith("files");
  });

  it("fires C2a pick_for_intake { kind: folder } on the choose-folder click (§5.3)", () => {
    const { getByRole } = render(<DropZone />);
    fireEvent.click(getByRole("button", { name: ui.dropzone_choose_folder }));
    expect(pickForIntake).toHaveBeenCalledTimes(1);
    expect(pickForIntake).toHaveBeenCalledWith("folder");
  });

  it("exposes both actions as native <button>s (Enter/Space activation is native button semantics, §5.3)", () => {
    const { getByRole } = render(<DropZone />);
    expect(getByRole("button", { name: ui.dropzone_prompt }).tagName).toBe("BUTTON");
    expect(getByRole("button", { name: ui.dropzone_choose_folder }).tagName).toBe("BUTTON");
  });
});

describe("DropZone — §5.10 Idle accelerators (Ctrl/⌘+O, Ctrl/⌘+Shift+O)", () => {
  beforeEach(() => {
    pickForIntake.mockReset();
    pickForIntake.mockResolvedValue(undefined);
  });

  // jsdom reports a non-macOS platform, so CmdOrCtrl resolves to Ctrl (the keymap.test.ts precedent).
  it("Ctrl+O fires C2a { kind: files } (keymap.openFilePicker)", () => {
    render(<DropZone />);
    fireEvent.keyDown(document, { key: "o", ctrlKey: true });
    expect(pickForIntake).toHaveBeenCalledTimes(1);
    expect(pickForIntake).toHaveBeenCalledWith("files");
  });

  it("Ctrl+Shift+O fires C2a { kind: folder } (keymap.chooseFolder), disambiguated from Ctrl+O by Shift", () => {
    render(<DropZone />);
    fireEvent.keyDown(document, { key: "o", ctrlKey: true, shiftKey: true });
    expect(pickForIntake).toHaveBeenCalledTimes(1);
    expect(pickForIntake).toHaveBeenCalledWith("folder");
  });

  it("a bare 'o' (no CmdOrCtrl) is not an accelerator (no picker fires)", () => {
    render(<DropZone />);
    fireEvent.keyDown(document, { key: "o" });
    expect(pickForIntake).not.toHaveBeenCalled();
  });

  it("unbinds the accelerator listener on unmount (no fire after the DropZone leaves Idle)", () => {
    const { unmount } = render(<DropZone />);
    unmount();
    fireEvent.keyDown(document, { key: "o", ctrlKey: true });
    expect(pickForIntake).not.toHaveBeenCalled();
  });
});

describe("DropZone — §5.4 DOM drag-over affordance (visual only, never a path source)", () => {
  beforeEach(() => {
    pickForIntake.mockReset();
    pickForIntake.mockResolvedValue(undefined);
  });

  it("lifts the affordance on dragenter and clears it when the pointer leaves the zone", () => {
    const { container } = render(<DropZone />);
    const zone = zoneOf(container);
    expect(zone.getAttribute("data-drag-active")).toBe("false");
    fireEvent.dragEnter(zone);
    expect(zone.getAttribute("data-drag-active")).toBe("true");
    fireEvent.dragLeave(zone);
    expect(zone.getAttribute("data-drag-active")).toBe("false");
  });

  it("keeps the surface lit across repeated dragover while a file hovers it", () => {
    const { container } = render(<DropZone />);
    const zone = zoneOf(container);
    fireEvent.dragEnter(zone);
    fireEvent.dragOver(zone);
    fireEvent.dragOver(zone);
    expect(zone.getAttribute("data-drag-active")).toBe("true");
  });

  it("clears the affordance on drop and NEVER ingests (the drop is core-side, §7.8.1/P3.77)", async () => {
    const { container } = render(<DropZone />);
    const zone = zoneOf(container);
    fireEvent.dragEnter(zone);
    fireEvent.drop(zone);
    await Promise.resolve(); // give any (forbidden) fire-and-forget picker call a chance to land
    expect(zone.getAttribute("data-drag-active")).toBe("false");
    expect(pickForIntake).not.toHaveBeenCalled();
  });

  it("preventDefaults dragover and drop so the WebView never navigates to a dropped file (§5.4)", () => {
    const { container } = render(<DropZone />);
    const zone = zoneOf(container);
    // fireEvent returns false when the handler called preventDefault on the cancelable event.
    expect(fireEvent.dragOver(zone)).toBe(false);
    expect(fireEvent.drop(zone)).toBe(false);
  });
});

describe("DropZone — §5.8 disabled guard (inert intake surface)", () => {
  beforeEach(() => {
    pickForIntake.mockReset();
    pickForIntake.mockResolvedValue(undefined);
  });

  it("disables both action buttons and fires no picker on a click", () => {
    const { getByRole } = render(<DropZone disabled />);
    const browse = getByRole("button", { name: ui.dropzone_prompt });
    const folder = getByRole("button", { name: ui.dropzone_choose_folder });
    // @testing-library/jest-dom is not a dependency here, so assert the disabled state via the DOM attribute.
    expect(browse.hasAttribute("disabled")).toBe(true);
    expect(folder.hasAttribute("disabled")).toBe(true);
    fireEvent.click(browse);
    fireEvent.click(folder);
    expect(pickForIntake).not.toHaveBeenCalled();
  });

  it("suppresses the §5.10 accelerators while disabled (no listener registered)", () => {
    render(<DropZone disabled />);
    fireEvent.keyDown(document, { key: "o", ctrlKey: true });
    fireEvent.keyDown(document, { key: "o", ctrlKey: true, shiftKey: true });
    expect(pickForIntake).not.toHaveBeenCalled();
  });

  it("does not lift the drag affordance while disabled", () => {
    const { container } = render(<DropZone disabled />);
    const zone = zoneOf(container);
    fireEvent.dragEnter(zone);
    fireEvent.dragOver(zone);
    expect(zone.getAttribute("data-drag-active")).toBe("false");
  });
});
