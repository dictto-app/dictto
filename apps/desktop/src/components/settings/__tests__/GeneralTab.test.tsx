import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GeneralTab } from "../GeneralTab";

// Default props factory
function defaultProps(overrides: Partial<Parameters<typeof GeneralTab>[0]> = {}) {
  return {
    settings: {} as Record<string, string>,
    onSave: vi.fn(),
    onOpenLanguageModal: vi.fn(),
    ...overrides,
  };
}

// SETT-01: Languages row renders with inline flag+name display in GeneralTab
describe("SETT-01: languages row display", () => {
  it("renders a 'Languages' label and description in the General tab", () => {
    render(<GeneralTab {...defaultProps()} />);

    expect(screen.getByText("Languages")).toBeInTheDocument();
    expect(screen.getByText("The languages you speak")).toBeInTheDocument();
  });

  it("shows Auto-detect text when languages setting is ['auto']", () => {
    render(
      <GeneralTab
        {...defaultProps({
          settings: { languages: JSON.stringify(["auto"]) },
        })}
      />
    );

    expect(screen.getByText("Auto-detect")).toBeInTheDocument();
  });

  it("shows inline flag and name when specific languages are set", () => {
    render(
      <GeneralTab
        {...defaultProps({
          settings: { languages: JSON.stringify(["es", "en"]) },
        })}
      />
    );

    // Should show both language names
    expect(screen.getByText("Spanish")).toBeInTheDocument();
    expect(screen.getByText("English")).toBeInTheDocument();
  });

  it("truncates with +N more when more than 2 languages are selected", () => {
    render(
      <GeneralTab
        {...defaultProps({
          settings: {
            languages: JSON.stringify(["es", "en", "fr", "de"]),
          },
        })}
      />
    );

    // Should show first two languages and +2 more
    expect(screen.getByText("Spanish")).toBeInTheDocument();
    expect(screen.getByText("English")).toBeInTheDocument();
    expect(screen.getByText(/\+2 more/)).toBeInTheDocument();
  });

  it("renders the Languages row between Sound effects and Push-to-talk", () => {
    const { container } = render(<GeneralTab {...defaultProps()} />);

    const allTexts = container.textContent || "";
    const soundIdx = allTexts.indexOf("Sound effects");
    const langIdx = allTexts.indexOf("Languages");
    const hotkeyIdx = allTexts.indexOf("Push-to-talk hotkey");

    // Languages should appear after Sound effects and before Push-to-talk
    expect(soundIdx).toBeLessThan(langIdx);
    expect(langIdx).toBeLessThan(hotkeyIdx);
  });
});

// SETT-02: Change button opens modal (calls onOpenLanguageModal callback)
describe("SETT-02: Change button opens modal", () => {
  it("renders a Change button in the Languages row", () => {
    render(<GeneralTab {...defaultProps()} />);

    // There are two "Change" buttons (Languages + Push-to-talk Coming Soon)
    // Find the one in the Languages row specifically
    const languagesLabel = screen.getByText("Languages");
    const languagesRow = languagesLabel.closest(".flex.items-center.justify-between")!;
    const changeBtn = languagesRow.querySelector("button")!;

    expect(changeBtn).toBeInTheDocument();
    expect(changeBtn.textContent).toBe("Change");
  });

  it("calls onOpenLanguageModal when Change button is clicked", async () => {
    const user = userEvent.setup();
    const onOpenLanguageModal = vi.fn();
    render(
      <GeneralTab {...defaultProps({ onOpenLanguageModal })} />
    );

    // Find the Change button within the Languages row
    const languagesLabel = screen.getByText("Languages");
    const languagesRow = languagesLabel.closest(".flex.items-center.justify-between")!;
    const changeBtn = languagesRow.querySelector("button")!;
    await user.click(changeBtn);

    expect(onOpenLanguageModal).toHaveBeenCalledTimes(1);
  });
});
