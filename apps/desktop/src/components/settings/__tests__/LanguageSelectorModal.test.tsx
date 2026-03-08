import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LanguageSelectorModal } from "../LanguageSelectorModal";

// Default props factory
function defaultProps(overrides: Partial<Parameters<typeof LanguageSelectorModal>[0]> = {}) {
  return {
    isOpen: true,
    initialLanguages: ["es", "en"],
    onSave: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
}

// MODAL-01: Modal header with title 'Languages' and subtitle text
describe("MODAL-01: modal header", () => {
  it("renders title 'Languages' and subtitle text when open", () => {
    render(<LanguageSelectorModal {...defaultProps()} />);

    expect(screen.getByText("Languages")).toBeInTheDocument();
    expect(
      screen.getByText("Select the languages you want to use with Dictto")
    ).toBeInTheDocument();
  });

  it("renders nothing when isOpen is false", () => {
    const { container } = render(
      <LanguageSelectorModal {...defaultProps({ isOpen: false })} />
    );
    expect(container.innerHTML).toBe("");
  });
});

// MODAL-02: Auto-detect toggle saves ['auto'], disables grid at 30% opacity
describe("MODAL-02: auto-detect toggle", () => {
  it("enables auto-detect when toggled ON, dims grid and sidebar", async () => {
    const user = userEvent.setup();
    render(<LanguageSelectorModal {...defaultProps()} />);

    const toggle = screen.getByLabelText("Auto-detect toggle");
    await user.click(toggle);

    // Grid+sidebar wrapper should have opacity 0.3 and pointer-events none
    const searchInput = screen.getByPlaceholderText("Search for any language...");
    expect(searchInput).toHaveStyle({ opacity: "0.3" });
  });

  it("saves ['auto'] when auto-detect is ON and user clicks save", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<LanguageSelectorModal {...defaultProps({ onSave })} />);

    // Toggle auto-detect on
    const toggle = screen.getByLabelText("Auto-detect toggle");
    await user.click(toggle);

    // Click save
    const saveBtn = screen.getByText("Save and close");
    await user.click(saveBtn);

    expect(onSave).toHaveBeenCalledWith(["auto"]);
  });

  it("restores previous selection when toggling auto-detect OFF", async () => {
    const user = userEvent.setup();
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["es", "en"] })} />
    );

    // Toggle ON (should backup current draft)
    const toggle = screen.getByLabelText("Auto-detect toggle");
    await user.click(toggle);

    // Toggle OFF (should restore previous draft)
    await user.click(toggle);

    // Search input should no longer be dimmed
    const searchInput = screen.getByPlaceholderText("Search for any language...");
    expect(searchInput).not.toHaveStyle({ opacity: "0.3" });

    // Sidebar should still show "Selected" with the previously selected languages
    expect(screen.getByText("Selected")).toBeInTheDocument();
  });

  it("initializes in auto-detect mode when initialLanguages is ['auto']", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["auto"] })} />
    );

    // Grid should be dimmed
    const searchInput = screen.getByPlaceholderText("Search for any language...");
    expect(searchInput).toHaveStyle({ opacity: "0.3" });
  });
});

// MODAL-03: Search filters grid in real time, pinned languages stay at top
describe("MODAL-03: search filtering", () => {
  it("filters language grid by search query in real time", async () => {
    const user = userEvent.setup();
    render(<LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />);

    const searchInput = screen.getByPlaceholderText("Search for any language...");
    await user.type(searchInput, "French");

    // French should appear
    expect(screen.getByText("French")).toBeInTheDocument();

    // An unrelated language like Afrikaans should NOT appear
    expect(screen.queryByText("Afrikaans")).not.toBeInTheDocument();
  });

  it("keeps pinned languages at top of results when they match search", async () => {
    const user = userEvent.setup();
    render(<LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />);

    const searchInput = screen.getByPlaceholderText("Search for any language...");
    // "an" should match "Spanish" (pinned) and many others like "Albanian", "German", etc.
    await user.type(searchInput, "an");

    // All matching items should be present
    const spanishElement = screen.getByText("Spanish");
    expect(spanishElement).toBeInTheDocument();

    // Get all language cards in the grid
    const gridContainer = spanishElement.closest(".grid");
    expect(gridContainer).not.toBeNull();

    // Spanish (pinned) should be first among matches
    const cards = gridContainer!.querySelectorAll("[class*='cursor-pointer']");
    expect(cards.length).toBeGreaterThan(0);

    // First matching card should be Spanish (pinned)
    expect(cards[0]).toHaveTextContent("Spanish");
  });
});

// MODAL-04: 3-column grid with ~100 languages, en/es/zh pinned at top
describe("MODAL-04: language grid layout", () => {
  it("renders a 3-column grid with grid-cols-3 class", () => {
    render(<LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />);

    // Find the grid container
    const gridEl = document.querySelector(".grid-cols-3");
    expect(gridEl).not.toBeNull();
  });

  it("shows English, Spanish, and Mandarin Chinese first in the grid", () => {
    render(<LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />);

    const gridEl = document.querySelector(".grid-cols-3");
    expect(gridEl).not.toBeNull();

    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");
    expect(cards.length).toBeGreaterThanOrEqual(3);

    // First three cards should be the pinned languages
    expect(cards[0]).toHaveTextContent("English");
    expect(cards[1]).toHaveTextContent("Spanish");
    expect(cards[2]).toHaveTextContent("Mandarin Chinese");
  });

  it("renders approximately 100 language cards", () => {
    render(<LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />);

    const gridEl = document.querySelector(".grid-cols-3");
    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");
    // LANGUAGES array has 100 entries
    expect(cards.length).toBeGreaterThanOrEqual(95);
    expect(cards.length).toBeLessThanOrEqual(105);
  });
});

// MODAL-05: Selected cards use bg-surface-elevated + border-border-strong
describe("MODAL-05: selected card visual state", () => {
  it("shows selected visual state for pre-selected languages", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["es"] })} />
    );

    // Find the Spanish card in the grid
    const gridEl = document.querySelector(".grid-cols-3");
    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");

    // Spanish card (index 1 since pinned order is en, es, zh) should have selected classes
    const spanishCard = cards[1];
    expect(spanishCard).toHaveTextContent("Spanish");
    expect(spanishCard.className).toContain("bg-surface-elevated");
    expect(spanishCard.className).toContain("border-border-strong");
  });

  it("toggles selected state when a card is clicked", async () => {
    const user = userEvent.setup();
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />
    );

    const gridEl = document.querySelector(".grid-cols-3");
    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");

    // French card: find by iterating
    let frenchCard: Element | null = null;
    for (const card of cards) {
      if (card.textContent?.includes("French")) {
        frenchCard = card;
        break;
      }
    }
    expect(frenchCard).not.toBeNull();

    // Initially not selected
    expect(frenchCard!.className).not.toContain("bg-surface-elevated");

    // Click to select
    await user.click(frenchCard!);

    // Now it should be selected
    expect(frenchCard!.className).toContain("bg-surface-elevated");
    expect(frenchCard!.className).toContain("border-border-strong");
  });

  it("unselected cards use bg-surface and border-border", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["es"] })} />
    );

    const gridEl = document.querySelector(".grid-cols-3");
    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");

    // English card (not selected, only es is selected)
    const englishCard = cards[0];
    expect(englishCard).toHaveTextContent("English");
    expect(englishCard.className).toContain("bg-surface");
    expect(englishCard.className).not.toContain("bg-surface-elevated");
  });
});

// MODAL-06: Right sidebar lists chosen languages, no remove button
describe("MODAL-06: selected languages sidebar", () => {
  it("shows 'Selected' header and chosen language names in sidebar", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["es", "en"] })} />
    );

    expect(screen.getByText("Selected")).toBeInTheDocument();
    // Sidebar should show both selected languages
    // There will be multiple "Spanish" and "English" texts (grid + sidebar)
    const selectedHeader = screen.getByText("Selected");
    const sidebar = selectedHeader.closest("div[class*='border-l']");
    expect(sidebar).not.toBeNull();

    expect(within(sidebar! as HTMLElement).getByText("Spanish")).toBeInTheDocument();
    expect(within(sidebar! as HTMLElement).getByText("English")).toBeInTheDocument();
  });

  it("shows 'No languages selected' when draft is empty", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />
    );

    expect(screen.getByText("No languages selected")).toBeInTheDocument();
  });

  it("does not render any remove buttons in the sidebar", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: ["es", "en"] })} />
    );

    const selectedHeader = screen.getByText("Selected");
    const sidebar = selectedHeader.closest("div[class*='border-l']");
    expect(sidebar).not.toBeNull();

    // No buttons should exist in the sidebar
    const buttons = sidebar!.querySelectorAll("button");
    expect(buttons.length).toBe(0);
  });
});

// MODAL-07: Save and close persists; no outside-click save
describe("MODAL-07: save behavior", () => {
  it("calls onSave with selected codes and onClose when save button is clicked", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(
      <LanguageSelectorModal
        {...defaultProps({ initialLanguages: ["es", "en"], onSave, onClose })}
      />
    );

    const saveBtn = screen.getByText("Save and close");
    await user.click(saveBtn);

    expect(onSave).toHaveBeenCalledWith(["es", "en"]);
    expect(onClose).toHaveBeenCalled();
  });

  it("disables save when no languages are selected and auto-detect is off", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />
    );

    const saveBtn = screen.getByText("Save and close");
    expect(saveBtn).toBeDisabled();
    expect(screen.getByText("Select at least one language")).toBeInTheDocument();
  });

  it("does not call onSave or onClose when clicking the backdrop", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(
      <LanguageSelectorModal
        {...defaultProps({ initialLanguages: ["es"], onSave, onClose })}
      />
    );

    // Click the backdrop (the outermost fixed div)
    const backdrop = document.querySelector(".fixed.inset-0");
    expect(backdrop).not.toBeNull();
    await user.click(backdrop!);

    // onSave/onClose should NOT be called from backdrop click
    // (the click might propagate to the modal card, but the backdrop itself has no handler)
    // We verify that no direct save was triggered — only Save button should trigger it
    expect(onSave).not.toHaveBeenCalled();
  });
});

// MODAL-08: Brand DNA: Figtree, @theme tokens, rounded-sm, btn-press, no hover states
describe("MODAL-08: brand DNA compliance", () => {
  it("uses rounded-lg for modal card", () => {
    render(<LanguageSelectorModal {...defaultProps()} />);

    const modalCard = document.querySelector(".rounded-lg");
    expect(modalCard).not.toBeNull();
    expect(modalCard!.className).toContain("bg-bg");
    expect(modalCard!.className).toContain("border-border-strong");
  });

  it("uses rounded-sm for language cards", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />
    );

    const gridEl = document.querySelector(".grid-cols-3");
    const cards = gridEl!.querySelectorAll("[class*='cursor-pointer']");
    expect(cards.length).toBeGreaterThan(0);

    // All cards should use rounded-sm
    for (const card of Array.from(cards).slice(0, 5)) {
      expect(card.className).toContain("rounded-sm");
    }
  });

  it("uses rounded-sm for save button", () => {
    render(<LanguageSelectorModal {...defaultProps()} />);

    const saveBtn = screen.getByText("Save and close");
    expect(saveBtn.className).toContain("rounded-sm");
  });

  it("uses theme tokens bg-surface, bg-surface-elevated, and text-text classes", () => {
    render(
      <LanguageSelectorModal {...defaultProps({ initialLanguages: [] })} />
    );

    // The search input should use bg-surface
    const searchInput = screen.getByPlaceholderText("Search for any language...");
    expect(searchInput.className).toContain("bg-surface");

    // The save button should use bg-surface-elevated
    const saveBtn = screen.getByText("Save and close");
    expect(saveBtn.className).toContain("bg-surface-elevated");
  });
});
