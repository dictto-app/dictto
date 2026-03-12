import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MicrophoneSelectorModal } from "../MicrophoneSelectorModal";

const DEVICE_LIST = [
  { name: "Headset Microphone", id: "endpoint-1", is_default: true, form_factor: "Bluetooth" },
  { name: "USB Mic", id: "endpoint-2", is_default: false, form_factor: "USB" },
];

function defaultProps(
  overrides: Partial<Parameters<typeof MicrophoneSelectorModal>[0]> = {}
) {
  return {
    isOpen: true,
    currentDevice: "auto-detect" as string,
    onSave: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "list_microphones") return Promise.resolve(DEVICE_LIST);
    return Promise.resolve(null);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

// DSEL-02: Modal renders header and device cards when open
describe("DSEL-02: modal visibility and content", () => {
  it("renders header 'Seleccionar micrófono' when isOpen is true", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("Seleccionar micrófono")).toBeInTheDocument();
  });

  it("renders subtitle 'Elige el dispositivo de entrada'", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("Elige el dispositivo de entrada")).toBeInTheDocument();
  });

  it("renders nothing when isOpen is false", () => {
    const { container } = render(
      <MicrophoneSelectorModal {...defaultProps({ isOpen: false })} />
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders device cards with device name", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("Headset Microphone")).toBeInTheDocument();
    expect(screen.getByText("USB Mic")).toBeInTheDocument();
  });

  it("renders device cards with form factor text badge", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("Bluetooth")).toBeInTheDocument();
    expect(screen.getByText("USB")).toBeInTheDocument();
  });
});

// DSEL-03: Auto-detect card is always first with subtitle
describe("DSEL-03: auto-detect card", () => {
  it("renders auto-detect card always first", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("Auto-detect")).toBeInTheDocument();
    expect(
      screen.getByText("Sigue el dispositivo predeterminado del sistema")
    ).toBeInTheDocument();
  });

  it("shows selected state on auto-detect card when currentDevice is 'auto-detect'", async () => {
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps({ currentDevice: "auto-detect" })} />);
    });

    // The auto-detect card should have selected styling
    const autoDetectCard = screen.getByText("Auto-detect").closest("[data-card]");
    expect(autoDetectCard).not.toBeNull();
    expect(autoDetectCard!.className).toContain("bg-surface-elevated");
  });

  it("does NOT show selected state on auto-detect card when a specific device is selected", async () => {
    await act(async () => {
      render(
        <MicrophoneSelectorModal
          {...defaultProps({ currentDevice: "Headset Microphone" })}
        />
      );
    });

    const autoDetectCard = screen.getByText("Auto-detect").closest("[data-card]");
    expect(autoDetectCard).not.toBeNull();
    expect(autoDetectCard!.className).not.toContain("bg-surface-elevated");
  });
});

// DSEL-04: Card click calls onSave; backdrop click calls onClose
describe("DSEL-04: click interactions", () => {
  it("clicking a device card calls onSave with the device name", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps({ onSave })} />);
    });

    const card = screen.getByText("USB Mic").closest("[data-card]")!;
    await user.click(card);

    expect(onSave).toHaveBeenCalledWith("USB Mic");
  });

  it("clicking the auto-detect card calls onSave with 'auto-detect'", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps({ onSave })} />);
    });

    const autoCard = screen.getByText("Auto-detect").closest("[data-card]")!;
    await user.click(autoCard);

    expect(onSave).toHaveBeenCalledWith("auto-detect");
  });

  it("clicking the backdrop calls onClose without onSave", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const onClose = vi.fn();
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps({ onSave, onClose })} />);
    });

    const backdrop = document.querySelector(".fixed.inset-0");
    expect(backdrop).not.toBeNull();
    await user.click(backdrop!);

    expect(onClose).toHaveBeenCalled();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("clicking modal card content does NOT trigger backdrop close", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps({ onClose })} />);
    });

    // Click the modal card itself (not backdrop)
    const modalCard = document.querySelector(".fixed.inset-0 > div");
    expect(modalCard).not.toBeNull();
    await user.click(modalCard!);

    expect(onClose).not.toHaveBeenCalled();
  });
});

// DSEL-05: Live refresh on audio-devices-changed event (200ms debounce)
describe("DSEL-05: live device refresh", () => {
  it("registers audio-devices-changed listener on mount", async () => {
    vi.useFakeTimers();
    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(vi.mocked(listen)).toHaveBeenCalledWith(
      "audio-devices-changed",
      expect.any(Function)
    );
  });

  it("re-fetches list_microphones after 200ms when audio-devices-changed fires", async () => {
    vi.useFakeTimers();
    const invokeCallsBefore = vi.mocked(invoke).mock.calls.length;

    let listenCallback: (() => void) | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(listen).mockImplementation((_event: any, cb: any) => {
      listenCallback = cb as () => void;
      return Promise.resolve(() => {});
    });

    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(listenCallback).not.toBeNull();

    // Fire the event
    act(() => {
      listenCallback!();
    });

    // Before 200ms, no additional invoke
    const invokeCallsMid = vi.mocked(invoke).mock.calls.filter(
      (c) => c[0] === "list_microphones"
    ).length;

    // Advance 200ms
    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    const invokeCallsAfter = vi.mocked(invoke).mock.calls.filter(
      (c) => c[0] === "list_microphones"
    ).length;

    // Should have been called again after the debounce
    expect(invokeCallsAfter).toBeGreaterThan(invokeCallsBefore);
    expect(invokeCallsAfter).toBeGreaterThan(invokeCallsMid);
  });
});

// DSEL-06: Auto-revert when selected device disappears from list
describe("DSEL-06: auto-revert on device disconnect", () => {
  it("calls onSave('auto-detect') when selected device is no longer in the list after refresh", async () => {
    vi.useFakeTimers();
    const onSave = vi.fn();

    let listenCallback: (() => void) | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(listen).mockImplementation((_event: any, cb: any) => {
      listenCallback = cb as () => void;
      return Promise.resolve(() => {});
    });

    await act(async () => {
      render(
        <MicrophoneSelectorModal
          {...defaultProps({ currentDevice: "USB Mic", onSave })}
        />
      );
    });

    // After device-changed event, the device list no longer contains "USB Mic"
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_microphones") {
        return Promise.resolve([
          { name: "Headset Microphone", id: "endpoint-1", is_default: true, form_factor: "Bluetooth" },
        ]);
      }
      return Promise.resolve(null);
    });

    act(() => {
      listenCallback!();
    });

    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    // Give promises time to resolve
    await act(async () => {
      await Promise.resolve();
    });

    expect(onSave).toHaveBeenCalledWith("auto-detect");
  });

  it("does NOT auto-revert when currentDevice is 'auto-detect'", async () => {
    vi.useFakeTimers();
    const onSave = vi.fn();

    let listenCallback: (() => void) | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(listen).mockImplementation((_event: any, cb: any) => {
      listenCallback = cb as () => void;
      return Promise.resolve(() => {});
    });

    await act(async () => {
      render(
        <MicrophoneSelectorModal
          {...defaultProps({ currentDevice: "auto-detect", onSave })}
        />
      );
    });

    // Refresh with a totally different list
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_microphones") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    act(() => {
      listenCallback!();
    });

    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(onSave).not.toHaveBeenCalled();
  });
});

// Empty state
describe("empty state", () => {
  it("shows 'No se encontraron micrófonos' when device list is empty", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_microphones") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(<MicrophoneSelectorModal {...defaultProps()} />);
    });

    expect(screen.getByText("No se encontraron micrófonos")).toBeInTheDocument();
    // Auto-detect card still visible
    expect(screen.getByText("Auto-detect")).toBeInTheDocument();
  });
});
