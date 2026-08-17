import { describe, it, expect } from "vitest";
import { dictionaries, resolveLocale, t } from "./index";
import { en } from "./en";

describe("i18n", () => {
  it("defaults to English", () => {
    expect(t("settings")).toBe("Settings");
  });

  it("returns Chinese strings for zh", () => {
    expect(t("settings", "zh")).toBe("设置");
    expect(t("interfaceLanguage", "zh")).toBe("界面语言");
    expect(t("barIdleTooltip", "zh")).toContain("{hotkey}");
    expect(t("barStopTooltip", "zh")).toBe("再按 {hotkey} 停止听写");
  });

  it("resolves unknown values to en", () => {
    expect(resolveLocale(undefined)).toBe("en");
    expect(resolveLocale("nope")).toBe("en");
    expect(resolveLocale("zh")).toBe("zh");
  });

  it("interpolates placeholders", () => {
    expect(t("moreLanguages", "en", { n: 2 })).toBe("+2 more");
    expect(t("autoDetectDevice", "en", { name: "Mic" })).toBe("Auto-detect (Mic)");
  });

  it("keeps the same keys in every dictionary", () => {
    const keys = Object.keys(en).sort();
    for (const [locale, dict] of Object.entries(dictionaries)) {
      expect(Object.keys(dict).sort(), locale).toEqual(keys);
    }
  });
});
