import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { GeneralTab } from "../components/settings/GeneralTab";
import { AudioTab } from "../components/settings/AudioTab";
import { ApiTab } from "../components/settings/TranscriptionTab";
import { LanguageSelectorModal } from "../components/settings/LanguageSelectorModal";

const tabs = [
  { id: "general", label: "General" },
  { id: "audio", label: "Audio" },
  { id: "api", label: "API" },
] as const;

type TabId = (typeof tabs)[number]["id"];

export function SettingsPage() {
  const [activeTab, setActiveTab] = useState<TabId>("general");
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKeyHint, setApiKeyHint] = useState("");
  const [isMaximized, setIsMaximized] = useState(false);
  const [isLanguageModalOpen, setIsLanguageModalOpen] = useState(false);

  useEffect(() => {
    loadSettings();
  }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setIsMaximized);
    let unlisten: (() => void) | undefined;
    win.onResized(() => {
      win.isMaximized().then(setIsMaximized);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  useEffect(() => {
    const unlisten = listen<{ key: string; value: string }>(
      "setting-changed",
      (event) => {
        setSettings((prev) => ({ ...prev, [event.payload.key]: event.payload.value }));
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function loadSettings() {
    try {
      const allSettings = await invoke<Record<string, string>>("get_all_settings");
      setSettings(allSettings);
      const keyExists = await invoke<boolean>("has_api_key");
      setHasApiKey(keyExists);
      if (keyExists) {
        const hint = await invoke<string>("get_api_key_hint");
        setApiKeyHint(hint);
      } else {
        // FRUN-01: Guide first-run users to the API tab
        setActiveTab("api");
      }
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function saveSetting(key: string, value: string) {
    try {
      await invoke("set_setting", { key, value });
      setSettings((prev) => ({ ...prev, [key]: value }));
    } catch (e) {
      console.error("Failed to save setting:", e);
    }
  }

  async function handleLanguageModalSave(languages: string[]) {
    const value = JSON.stringify(languages);
    await saveSetting("languages", value);
    setIsLanguageModalOpen(false);
  }

  async function saveApiKey(apiKey: string) {
    try {
      await invoke("set_api_key", { apiKey });
      setHasApiKey(true);
      const hint = await invoke<string>("get_api_key_hint");
      setApiKeyHint(hint);
    } catch (e) {
      console.error("Failed to save API key:", e);
    }
  }

  async function removeApiKey() {
    try {
      await invoke("remove_api_key");
      setHasApiKey(false);
      setApiKeyHint("");
    } catch (e) {
      console.error("Failed to remove API key:", e);
    }
  }

  const handleTabClick = (e: React.MouseEvent<HTMLButtonElement>, tabId: TabId) => {
    const btn = e.currentTarget;
    btn.classList.remove("btn-press");
    void btn.offsetWidth;
    btn.classList.add("btn-press");
    setActiveTab(tabId);
  };

  const handlePress = (
    e: React.MouseEvent<HTMLButtonElement>,
    action: () => void
  ) => {
    const btn = e.currentTarget;
    btn.classList.remove("btn-press");
    void btn.offsetWidth;
    btn.classList.add("btn-press");
    action();
  };

  const win = getCurrentWindow();

  return (
    <div className="flex flex-col h-screen bg-bg text-text">
      {/* Integrated header: logo + title + tabs + window controls */}
      <div
        data-tauri-drag-region
        className="flex items-stretch border-b border-border bg-bg shrink-0"
      >
        {/* Left: Logo + Title + Tabs — vertically centered with padding */}
        <div
          className="flex items-center gap-5 pl-7 py-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          <svg
            className="w-5 h-5 text-text shrink-0"
            viewBox="0 0 120 120"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            style={{ pointerEvents: "none" }}
          >
            <defs>
              <mask id="waveform-sm">
                <rect width="120" height="120" fill="white" />
                <rect x="44" y="36" width="9" height="48" rx="4.5" fill="black" />
                <rect x="56" y="28" width="9" height="64" rx="4.5" fill="black" />
                <rect x="68" y="38" width="9" height="44" rx="4.5" fill="black" />
              </mask>
            </defs>
            <path
              d="M26 16H58C88 16 106 36 106 60C106 84 88 104 58 104H26V16Z"
              fill="currentColor"
              mask="url(#waveform-sm)"
            />
          </svg>
          <span className="text-base font-bold tracking-tight text-text" style={{ pointerEvents: "none" }}>
            Settings
          </span>

          {/* Tab pills */}
          <div
            className="flex items-center gap-1 ml-auto"
            style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
          >
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={(e) => handleTabClick(e, tab.id)}
                className={`px-3.5 py-1.5 text-xs font-semibold rounded-sm flex items-center gap-1.5 ${
                  activeTab === tab.id
                    ? "bg-surface-elevated text-text"
                    : "text-text-tertiary"
                }`}
              >
                {tab.label}
                {tab.id === "api" && !hasApiKey && (
                  <span className="inline-flex items-center px-1.5 py-0.5 rounded-xs text-[10px] font-bold leading-none bg-danger/15 text-danger border border-danger/25">
                    !
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>

        {/* Right: Window controls — full bleed (stretch full header height) */}
        <div
          className="flex shrink-0"
          style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
        >
          <button
            className="flex items-center justify-center w-[46px] self-stretch text-text-tertiary hover:bg-border-strong hover:text-text-secondary cursor-default"
            onClick={(e) => handlePress(e, () => { win.minimize(); })}
            aria-label="Minimizar"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5">
              <line x1="0" y1="5" x2="10" y2="5" />
            </svg>
          </button>
          <button
            className="flex items-center justify-center w-[46px] self-stretch text-text-tertiary hover:bg-border-strong hover:text-text-secondary cursor-default"
            onClick={(e) => handlePress(e, () => { win.toggleMaximize(); })}
            aria-label={isMaximized ? "Restaurar" : "Maximizar"}
          >
            {isMaximized ? (
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="0.75" y="2.75" width="6.5" height="6.5" />
                <polyline points="2.75,2.75 2.75,0.75 9.25,0.75 9.25,7.25 7.25,7.25" />
              </svg>
            ) : (
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="0.75" y="0.75" width="8.5" height="8.5" />
              </svg>
            )}
          </button>
          <button
            className="flex items-center justify-center w-[46px] self-stretch text-text-tertiary hover:bg-danger hover:text-text cursor-default"
            onClick={(e) => handlePress(e, () => { win.close(); })}
            aria-label="Cerrar"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5">
              <line x1="0" y1="0" x2="10" y2="10" />
              <line x1="10" y1="0" x2="0" y2="10" />
            </svg>
          </button>
        </div>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto p-7">
        {activeTab === "general" && (
          <GeneralTab
            settings={settings}
            onSave={saveSetting}
            onOpenLanguageModal={() => setIsLanguageModalOpen(true)}
          />
        )}
        {activeTab === "audio" && (
          <AudioTab settings={settings} onSave={saveSetting} />
        )}
        {activeTab === "api" && (
          <ApiTab
            hasApiKey={hasApiKey}
            apiKeyHint={apiKeyHint}
            onSaveApiKey={saveApiKey}
            onRemoveApiKey={removeApiKey}
          />
        )}
      </div>

      {/* Language Selector Modal — backdrop has no onClick (MODAL-07: no save on outside-click) */}
      <LanguageSelectorModal
        isOpen={isLanguageModalOpen}
        initialLanguages={(() => {
          try { return JSON.parse(settings.languages || "[]"); }
          catch { return []; }
        })()}
        onSave={handleLanguageModalSave}
        onClose={() => setIsLanguageModalOpen(false)}
      />
    </div>
  );
}
