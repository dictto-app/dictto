import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SettingsPage } from "../Settings";

// Type the mocks
const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  vi.clearAllMocks();

  // Default: get_all_settings returns languages setting
  mockInvoke.mockImplementation(async (cmd: string, _args?: unknown) => {
    if (cmd === "get_all_settings") {
      return { languages: JSON.stringify(["es", "en"]) } as Record<string, string>;
    }
    if (cmd === "has_api_key") return false;
    if (cmd === "set_setting") return null;
    return null;
  });

  // Default: listen returns an unlisten function
  mockListen.mockResolvedValue(() => {});
});

// SETT-03: Row updates live after modal save via setting-changed event
describe("SETT-03: row updates live after modal save", () => {
  it("registers a setting-changed event listener on mount", async () => {
    await act(async () => {
      render(<SettingsPage />);
    });

    // listen should have been called with "setting-changed"
    expect(mockListen).toHaveBeenCalledWith(
      "setting-changed",
      expect.any(Function)
    );
  });

  it("updates the languages display when setting-changed event fires", async () => {
    // Capture the setting-changed callback
    let settingChangedCallback: ((event: { payload: { key: string; value: string } }) => void) | null = null;

    mockListen.mockImplementation(async (eventName: string, handler: unknown) => {
      if (eventName === "setting-changed") {
        settingChangedCallback = handler as typeof settingChangedCallback;
      }
      return () => {};
    });

    await act(async () => {
      render(<SettingsPage />);
    });

    // Wait for initial settings to load
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Verify initial state shows Spanish and English
    expect(screen.getByText("Spanish")).toBeInTheDocument();
    expect(screen.getByText("English")).toBeInTheDocument();

    // Simulate a setting-changed event (e.g., after modal save changes languages to French+German)
    expect(settingChangedCallback).not.toBeNull();

    await act(async () => {
      settingChangedCallback!({
        payload: {
          key: "languages",
          value: JSON.stringify(["fr", "de"]),
        },
      });
    });

    // The display should now show French and German
    expect(screen.getByText("French")).toBeInTheDocument();
    expect(screen.getByText("German")).toBeInTheDocument();
  });

  it("opens the language modal when Change is clicked and closes after save", async () => {
    const user = userEvent.setup();

    await act(async () => {
      render(<SettingsPage />);
    });

    // Wait for settings to load
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Click the Change button in the Languages row (not the ComingSoon one)
    const languagesLabel = screen.getByText("The languages you speak");
    const languagesRow = languagesLabel.closest(".flex.items-center.justify-between")!;
    const changeBtn = languagesRow.querySelector("button")!;
    await user.click(changeBtn);

    // Modal should be open - check for the "Languages" heading inside the modal
    // (There's also "Languages" label in GeneralTab, so look for the modal-specific subtitle)
    expect(
      screen.getByText("Select the languages you want to use with Dictto")
    ).toBeInTheDocument();

    // Click Save and close
    const saveBtn = screen.getByText("Save and close");
    await user.click(saveBtn);

    // After save, set_setting should have been called
    expect(mockInvoke).toHaveBeenCalledWith("set_setting", {
      key: "languages",
      value: expect.any(String),
    });
  });
});
