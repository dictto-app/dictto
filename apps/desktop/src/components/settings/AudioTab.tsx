import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ComingSoon } from "./ComingSoon";

interface Props {
  settings: Record<string, string>;
  onSave: (key: string, value: string) => void;
  onOpenMicModal: () => void;
}

function MicrophoneInlineDisplay({
  microphoneDevice,
  currentDeviceName,
}: {
  microphoneDevice: string | undefined;
  currentDeviceName: string;
}) {
  const isAutoDetect = !microphoneDevice || microphoneDevice === "auto-detect";
  if (isAutoDetect) {
    return (
      <span className="text-xs text-text-secondary">
        {currentDeviceName
          ? `Auto-detect (${currentDeviceName.replace(/\s*\([^)]*\)\s*$/, "")})`
          : "Auto-detect"}
      </span>
    );
  }
  return <span className="text-xs text-text-secondary">{microphoneDevice}</span>;
}

export function AudioTab({ settings, onSave: _onSave, onOpenMicModal }: Props) {
  const [currentDeviceName, setCurrentDeviceName] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  async function fetchCurrentDevice() {
    try {
      const name = await invoke<string>("get_current_microphone");
      setCurrentDeviceName(name);
    } catch (e) {
      console.error("Failed to get current microphone:", e);
    }
  }

  // Fetch current device name on mount
  useEffect(() => {
    fetchCurrentDevice();
  }, []);

  // DSEL-05: Listen for audio-devices-changed with 200ms debounce
  useEffect(() => {
    const unlistenPromise = listen("audio-devices-changed", () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        fetchCurrentDevice();
      }, 200);
    });

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  return (
    <div className="space-y-6">
      {/* Microphone — functional */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">Microphone</p>
          <p className="text-xs text-text-tertiary">
            The input device for recording
          </p>
        </div>
        <div className="flex items-center gap-3">
          <MicrophoneInlineDisplay
            microphoneDevice={settings.microphone_device}
            currentDeviceName={currentDeviceName}
          />
          <button
            onClick={(e) => {
              const btn = e.currentTarget;
              btn.classList.remove("btn-press");
              void btn.offsetWidth;
              btn.classList.add("btn-press");
              onOpenMicModal();
            }}
            className="px-3 py-1.5 bg-surface-elevated border border-border-strong rounded-sm text-xs font-medium text-text-secondary"
          >
            Change
          </button>
        </div>
      </div>

      {/* Noise suppression — Coming Soon */}
      <ComingSoon>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-text-secondary">Noise suppression</p>
            <p className="text-xs text-text-tertiary">Filter background noise during recording</p>
          </div>
          <div className="relative w-10 h-5 rounded-full bg-surface-elevated">
            <span className="absolute top-0.5 left-0.5 w-4 h-4 bg-text rounded-full" />
          </div>
        </div>
      </ComingSoon>

      {/* Auto-detect silence — Coming Soon */}
      <ComingSoon>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-text-secondary">Auto-detect silence</p>
            <p className="text-xs text-text-tertiary">Automatically stop recording after silence</p>
          </div>
          <div className="relative w-10 h-5 rounded-full bg-surface-elevated">
            <span className="absolute top-0.5 left-0.5 w-4 h-4 bg-text rounded-full" />
          </div>
        </div>
      </ComingSoon>
    </div>
  );
}
