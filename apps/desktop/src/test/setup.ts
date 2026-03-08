import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";
import React from "react";

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

// Mock @tauri-apps/api/event
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock @tauri-apps/api/window
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    label: "main",
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  })),
}));

// Mock country-flag-icons/react/3x2 with explicit named exports for all country codes used
// This creates a simple React component for each country code
function makeFlagComponent(code: string) {
  const component = (props: Record<string, unknown>) =>
    React.createElement("span", {
      ...props,
      "data-testid": `flag-${code}`,
    }, code);
  component.displayName = `Flag${code}`;
  return component;
}

const countryCodes = [
  "AF", "AL", "AM", "AZ", "BA", "BD", "BG", "BR", "BY", "CD",
  "CN", "CZ", "DE", "DK", "EE", "ES", "ET", "FI", "FO", "FR",
  "GB", "GE", "GR", "HK", "HR", "HT", "HU", "ID", "IL", "IN",
  "IR", "IS", "IT", "JP", "KH", "KR", "KZ", "LA", "LK", "LT",
  "LU", "LV", "MG", "MK", "MM", "MN", "MT", "MY", "NG", "NL",
  "NO", "NP", "NZ", "PH", "PK", "PL", "RO", "RS", "RU", "SA",
  "SE", "SI", "SK", "SO", "TH", "TJ", "TM", "TR", "TZ", "UA",
  "US", "UZ", "VN", "ZA", "ZW",
];

const flagMocks: Record<string, ReturnType<typeof makeFlagComponent>> = {};
for (const code of countryCodes) {
  flagMocks[code] = makeFlagComponent(code);
}

vi.mock("country-flag-icons/react/3x2", () => flagMocks);
