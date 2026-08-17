import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RecordingBar } from "../components/recording-bar/RecordingBar";
import { I18nProvider, resolveLocale, type Locale } from "../i18n";

interface SettingChangedPayload {
  key: string;
  value: string;
}

export function RecordingBarPage() {
  const [barVisibleIdle, setBarVisibleIdle] = useState(true);
  const [barOpacity, setBarOpacity] = useState(0.9);
  const [locale, setLocale] = useState<Locale>("en");

  useEffect(() => {
    invoke<string | null>("get_setting", { key: "bar_visible_idle" })
      .then((val) => setBarVisibleIdle(val !== "false"))
      .catch(console.error);

    invoke<string | null>("get_setting", { key: "bar_opacity" })
      .then((val) => setBarOpacity(val ? parseFloat(val) : 0.9))
      .catch(console.error);

    invoke<string | null>("get_setting", { key: "ui_locale" })
      .then((val) => setLocale(resolveLocale(val ?? undefined)))
      .catch(console.error);

    const unlisten = listen<SettingChangedPayload>(
      "setting-changed",
      (event) => {
        if (event.payload.key === "bar_visible_idle") {
          setBarVisibleIdle(event.payload.value !== "false");
        }
        if (event.payload.key === "bar_opacity") {
          setBarOpacity(parseFloat(event.payload.value));
        }
        if (event.payload.key === "ui_locale") {
          setLocale(resolveLocale(event.payload.value));
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <I18nProvider locale={locale}>
      <RecordingBar barVisibleIdle={barVisibleIdle} barOpacity={barOpacity} />
    </I18nProvider>
  );
}
