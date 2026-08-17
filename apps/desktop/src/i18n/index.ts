import { createContext, createElement, useContext, type ReactNode } from "react";
import { en, type MessageKey } from "./en";
import { zh } from "./zh";

export type { MessageKey };

/** Register a new UI language by adding a dictionary here and an entry in localeOptions. */
export const dictionaries = { en, zh };

export type Locale = keyof typeof dictionaries;

/** Native labels stay untranslated so the language picker remains recognizable. */
export const localeOptions: { id: Locale; nativeLabel: string }[] = [
  { id: "en", nativeLabel: "English" },
  { id: "zh", nativeLabel: "中文" },
];

export function resolveLocale(value: string | undefined): Locale {
  if (value && value in dictionaries) return value as Locale;
  return "en";
}

export function t(
  key: MessageKey,
  locale: Locale = "en",
  vars?: Record<string, string | number>
): string {
  let text: string = dictionaries[locale][key] ?? en[key];
  if (!vars) return text;
  for (const [name, value] of Object.entries(vars)) {
    text = text.split(`{${name}}`).join(String(value));
  }
  return text;
}

const LocaleContext = createContext<Locale>("en");

export function I18nProvider({
  locale,
  children,
}: {
  locale: Locale;
  children: ReactNode;
}) {
  return createElement(LocaleContext.Provider, { value: locale }, children);
}

export function useT() {
  const locale = useContext(LocaleContext);
  return (key: MessageKey, vars?: Record<string, string | number>) =>
    t(key, locale, vars);
}
