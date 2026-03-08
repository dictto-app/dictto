import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ComingSoon } from "./ComingSoon";

interface MicrophoneInfo {
  name: string;
  is_default: boolean;
}

interface Props {
  settings: Record<string, string>;
  onSave: (key: string, value: string) => void;
}

export function AudioTab({ settings, onSave }: Props) {
  const [microphones, setMicrophones] = useState<MicrophoneInfo[]>([]);

  useEffect(() => {
    invoke<MicrophoneInfo[]>("list_microphones")
      .then(setMicrophones)
      .catch(console.error);
  }, []);

  return (
    <div className="space-y-6">
      {/* Microphone — functional */}
      <div>
        <label className="block text-sm font-medium text-text-secondary mb-2">
          Microphone
        </label>
        <select
          value={settings.microphone_device || "default"}
          onChange={(e) => onSave("microphone_device", e.target.value)}
          className="w-full px-3 py-2 bg-surface border border-border-strong rounded-sm text-sm text-text"
        >
          <option value="default">System Default</option>
          {microphones.map((mic) => (
            <option key={mic.name} value={mic.name}>
              {mic.name} {mic.is_default ? "(Default)" : ""}
            </option>
          ))}
        </select>
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
