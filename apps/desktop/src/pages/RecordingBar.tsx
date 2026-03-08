import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RecordingBar } from "../components/recording-bar/RecordingBar";

interface SettingChangedPayload {
  key: string;
  value: string;
}

export function RecordingBarPage() {
  const [barVisibleIdle, setBarVisibleIdle] = useState(true);
  const [barOpacity, setBarOpacity] = useState(0.9);

  useEffect(() => {
    invoke<string | null>("get_setting", { key: "bar_visible_idle" })
      .then((val) => setBarVisibleIdle(val !== "false"))
      .catch(console.error);

    invoke<string | null>("get_setting", { key: "bar_opacity" })
      .then((val) => setBarOpacity(val ? parseFloat(val) : 0.9))
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
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <RecordingBar barVisibleIdle={barVisibleIdle} barOpacity={barOpacity} />
  );
}
