import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface MicrophoneInfo {
  name: string;
  id: string;
  is_default: boolean;
  form_factor: string;
}

interface MicrophoneSelectorModalProps {
  isOpen: boolean;
  currentDevice: string; // "auto-detect" or device name
  onSave: (value: string) => void;
  onClose: () => void;
}

export function MicrophoneSelectorModal({
  isOpen,
  currentDevice,
  onSave,
  onClose,
}: MicrophoneSelectorModalProps) {
  const [devices, setDevices] = useState<MicrophoneInfo[]>([]);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  async function refreshDevices() {
    try {
      const list = await invoke<MicrophoneInfo[]>("list_microphones");
      setDevices(list);
      // DSEL-06: auto-revert if selected device disappeared
      if (
        currentDevice !== "auto-detect" &&
        !list.some((d) => d.name === currentDevice)
      ) {
        onSave("auto-detect");
      }
    } catch (e) {
      console.error("Failed to list microphones:", e);
    }
  }

  // Fetch device list on mount when modal is open
  useEffect(() => {
    if (isOpen) {
      refreshDevices();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  // DSEL-05: Listen for audio-devices-changed with 200ms debounce
  useEffect(() => {
    if (!isOpen) return;

    const unlistenPromise = listen("audio-devices-changed", () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        refreshDevices();
      }, 200);
    });

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      unlistenPromise.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, currentDevice]);

  if (!isOpen) return null;

  function handleCardClick(
    e: React.MouseEvent<HTMLDivElement>,
    value: string
  ) {
    const el = e.currentTarget;
    el.classList.remove("btn-press");
    void el.offsetWidth;
    el.classList.add("btn-press");
    onSave(value);
  }

  const isAutoDetect = currentDevice === "auto-detect";
  const defaultDevice = devices.find((d) => d.is_default);
  // Strip driver suffix: "Línea de entrada (Realtek USB Audio)" → "Línea de entrada"
  const defaultShortName = defaultDevice?.name.replace(/\s*\([^)]*\)\s*$/, "");

  return (
    // Backdrop
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(18, 17, 15, 0.75)" }}
      onClick={onClose}
    >
      {/* Modal card */}
      <div
        className="flex flex-col bg-bg border border-border-strong rounded-lg overflow-hidden"
        style={{
          width: "480px",
          maxWidth: "calc(100vw - 40px)",
          maxHeight: "calc(100vh - 40px)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-5 py-4 border-b border-border shrink-0">
          <h2 className="text-base font-bold text-text">Select microphone</h2>
          <p className="text-xs text-text-tertiary mt-0.5">
            Choose the input device
          </p>
        </div>

        {/* Scrollable card list */}
        <div className="overflow-y-auto p-3 space-y-2">
          {/* Auto-detect card — always first */}
          <div
            data-card
            onClick={(e) => handleCardClick(e, "auto-detect")}
            className={`px-3 py-2.5 rounded-sm cursor-pointer select-none border ${
              isAutoDetect
                ? "bg-surface-elevated border-border-strong"
                : "bg-surface border-border"
            }`}
          >
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-text">
                Auto-detect{defaultShortName ? ` (${defaultShortName})` : ""}
              </span>
              <span className="text-xs text-accent font-medium">Auto</span>
            </div>
            <p className="text-xs text-text-tertiary mt-0.5">
              Follows the system default device
            </p>
          </div>

          {/* Device cards */}
          {devices.length === 0 ? (
            <div className="py-6 text-center">
              <p className="text-sm text-text-tertiary">No microphones found</p>
            </div>
          ) : (
            devices.map((mic) => {
              const isSelected = currentDevice === mic.name;
              return (
                <div
                  key={mic.id}
                  data-card
                  onClick={(e) => handleCardClick(e, mic.name)}
                  className={`flex items-center justify-between px-3 py-2.5 rounded-sm cursor-pointer select-none border ${
                    isSelected
                      ? "bg-surface-elevated border-border-strong"
                      : "bg-surface border-border"
                  }`}
                >
                  <span className="text-sm text-text truncate">{mic.name}</span>
                  <span className="text-xs text-text-tertiary ml-3 shrink-0">
                    {mic.form_factor}
                  </span>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
