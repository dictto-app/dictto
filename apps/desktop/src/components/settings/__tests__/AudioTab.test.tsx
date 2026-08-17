import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AudioTab } from "../AudioTab";

function defaultProps(
  overrides: Partial<Parameters<typeof AudioTab>[0]> = {}
) {
  return {
    settings: {} as Record<string, string>,
    onSave: vi.fn(),
    onOpenMicModal: vi.fn(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "get_current_microphone") return Promise.resolve("Headset Microphone");
    if (cmd === "list_microphones") return Promise.resolve([]);
    return Promise.resolve(null);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

// DSEL-01: Inline row with label, description, device name, and Change button
describe("DSEL-01: inline microphone row display", () => {
  it("renders 'Microphone' label", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(screen.getByText("Microphone")).toBeInTheDocument();
  });

  it("renders description 'The input device for recording'", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(
      screen.getByText("The input device for recording")
    ).toBeInTheDocument();
  });

  it("renders 'Change' button", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(screen.getByText("Change")).toBeInTheDocument();
  });

  it("shows 'Auto-detect (Headset Microphone)' when microphone_device is 'auto-detect'", async () => {
    await act(async () => {
      render(
        <AudioTab
          {...defaultProps({
            settings: { microphone_device: "auto-detect" },
          })}
        />
      );
    });

    // get_current_microphone returns "Headset Microphone"
    expect(screen.getByText("Auto-detect (Headset Microphone)")).toBeInTheDocument();
  });

  it("shows 'Auto-detect (Headset Microphone)' when microphone_device is undefined/empty", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps({ settings: {} })} />);
    });

    expect(screen.getByText("Auto-detect (Headset Microphone)")).toBeInTheDocument();
  });

  it("shows device name directly when microphone_device is a specific name", async () => {
    await act(async () => {
      render(
        <AudioTab
          {...defaultProps({
            settings: { microphone_device: "USB Mic" },
          })}
        />
      );
    });

    expect(screen.getByText("USB Mic")).toBeInTheDocument();
  });
});

// DSEL-01: Change button calls onOpenMicModal
describe("DSEL-01: Change button callback", () => {
  it("calls onOpenMicModal when Change is clicked", async () => {
    const user = userEvent.setup();
    const onOpenMicModal = vi.fn();
    await act(async () => {
      render(<AudioTab {...defaultProps({ onOpenMicModal })} />);
    });

    const btn = screen.getByText("Change");
    await user.click(btn);

    expect(onOpenMicModal).toHaveBeenCalledTimes(1);
  });
});

// DSEL-05: Live refresh on audio-devices-changed
describe("DSEL-05: live refresh on device change", () => {
  it("registers audio-devices-changed listener on mount", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(vi.mocked(listen)).toHaveBeenCalledWith(
      "audio-devices-changed",
      expect.any(Function)
    );
  });

  it("re-invokes get_current_microphone after 200ms when audio-devices-changed fires", async () => {
    vi.useFakeTimers();

    let listenCallback: (() => void) | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(listen).mockImplementation((_event: any, cb: any) => {
      listenCallback = cb as () => void;
      return Promise.resolve(() => {});
    });

    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    const initialInvokeCalls = vi.mocked(invoke).mock.calls.filter(
      (c) => c[0] === "get_current_microphone"
    ).length;

    expect(listenCallback).not.toBeNull();

    act(() => {
      listenCallback!();
    });

    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    await act(async () => {
      await Promise.resolve();
    });

    const afterInvokeCalls = vi.mocked(invoke).mock.calls.filter(
      (c) => c[0] === "get_current_microphone"
    ).length;

    expect(afterInvokeCalls).toBeGreaterThan(initialInvokeCalls);
  });
});

// Existing sections preserved
describe("preserved sections", () => {
  it("still renders 'Noise suppression' text", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(screen.getByText("Noise suppression")).toBeInTheDocument();
  });

  it("still renders 'Auto-detect silence' text", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    expect(screen.getByText("Auto-detect silence")).toBeInTheDocument();
  });

  it("does NOT render a select element (old dropdown removed)", async () => {
    await act(async () => {
      render(<AudioTab {...defaultProps()} />);
    });

    const selectEl = document.querySelector("select");
    expect(selectEl).toBeNull();
  });
});
