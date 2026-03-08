import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SettingsPage } from "./pages/Settings";
import { RecordingBarPage } from "./pages/RecordingBar";

function App() {
  const [windowLabel, setWindowLabel] = useState<string | null>(null);

  useEffect(() => {
    const label = getCurrentWindow().label;
    setWindowLabel(label);
  }, []);

  if (!windowLabel) {
    return null;
  }

  switch (windowLabel) {
    case "recording-bar":
      return <RecordingBarPage />;
    case "main":
    default:
      return <SettingsPage />;
  }
}

export default App;
