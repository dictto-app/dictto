import { useState, useEffect } from "react";

interface ApiTabProps {
  hasApiKey: boolean;
  apiKeyHint: string;
  onSaveApiKey: (key: string) => void;
  onRemoveApiKey: () => Promise<void>;
  transcriptionEngine: string;
  parakeetModelDir: string;
  onSaveTranscriptionEngine: (engine: string) => void;
  onSaveParakeetModelDir: (dir: string) => void;
}

type KeyView = "display" | "input" | "update";

export function ApiTab({
  hasApiKey,
  apiKeyHint,
  onSaveApiKey,
  onRemoveApiKey,
  transcriptionEngine,
  parakeetModelDir,
  onSaveTranscriptionEngine,
  onSaveParakeetModelDir,
}: ApiTabProps) {
  const [view, setView] = useState<KeyView>(hasApiKey ? "display" : "input");
  const [apiKey, setApiKey] = useState("");
  const [showRemoveConfirm, setShowRemoveConfirm] = useState(false);
  const [modelDirInput, setModelDirInput] = useState(parakeetModelDir);

  useEffect(() => {
    setModelDirInput(parakeetModelDir);
  }, [parakeetModelDir]);

  useEffect(() => {
    setView(hasApiKey ? "display" : "input");
  }, [hasApiKey]);

  const handlePress = (e: React.MouseEvent<HTMLButtonElement>) => {
    const btn = e.currentTarget;
    btn.classList.remove("btn-press");
    void btn.offsetWidth;
    btn.classList.add("btn-press");
  };

  const isLocalEngine = transcriptionEngine === "parakeet_local";

  // ENGINE-01: Engine picker — shown above the OpenAI key section regardless
  // of which key-entry view is active below. Selecting "parakeet_local"
  // requires a downloaded ONNX model directory (see README "Local
  // transcription" section) but needs no API key and sends no audio anywhere.
  const enginePicker = (
    <div className="mb-5 pb-5 border-b border-border">
      <div className="text-xs font-semibold text-text-secondary mb-2">
        Transcription Engine
      </div>
      <div className="flex gap-2 mb-3">
        <button
          onClick={(e) => {
            handlePress(e);
            onSaveTranscriptionEngine("openai_api");
          }}
          className={`flex-1 px-3.5 py-2.5 rounded-sm text-xs font-semibold border ${
            !isLocalEngine
              ? "bg-surface-elevated border-border-strong text-text"
              : "bg-surface border-border text-text-tertiary"
          }`}
        >
          OpenAI Whisper (cloud)
        </button>
        <button
          onClick={(e) => {
            handlePress(e);
            onSaveTranscriptionEngine("parakeet_local");
          }}
          className={`flex-1 px-3.5 py-2.5 rounded-sm text-xs font-semibold border ${
            isLocalEngine
              ? "bg-surface-elevated border-border-strong text-text"
              : "bg-surface border-border text-text-tertiary"
          }`}
        >
          Parakeet (local, offline)
        </button>
      </div>

      {isLocalEngine && (
        <div>
          <div className="flex items-center gap-2.5">
            <input
              type="text"
              value={modelDirInput}
              onChange={(e) => setModelDirInput(e.target.value)}
              placeholder="C:\path\to\parakeet-tdt-0.6b-v3-onnx"
              className="flex-1 px-3.5 py-2.5 bg-surface border border-border rounded-sm text-[13px] text-text placeholder:text-text-tertiary"
            />
            <button
              onClick={(e) => {
                handlePress(e);
                onSaveParakeetModelDir(modelDirInput.trim());
              }}
              className="px-4 py-2.5 bg-text text-bg rounded-md text-xs font-semibold shrink-0"
            >
              Save Path
            </button>
          </div>
          <p className="text-[11px] text-text-tertiary mt-2 leading-relaxed">
            Runs 100% on-device via ONNX Runtime — no API key, no network call,
            audio never leaves this PC. Download a Parakeet TDT ONNX model
            bundle first (see README "Local transcription"), then point this
            at the folder containing model.onnx.
          </p>
        </div>
      )}
    </div>
  );

  // No key — input with Save Key button
  if (view === "input") {
    return (
      <div>
        {enginePicker}
        <div className="flex items-center gap-2 mb-2">
          <span className="text-xs font-semibold text-text-secondary">
            OpenAI API Key
          </span>
          <span className="inline-flex items-center px-2 py-0.5 rounded-xs text-[10px] font-bold leading-none bg-danger/15 text-danger border border-danger/25">
            {isLocalEngine ? "Optional" : "Required"}
          </span>
        </div>
        <div className="flex items-center gap-2.5">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-proj-..."
            className="flex-1 px-3.5 py-2.5 bg-surface border border-border rounded-sm text-[13px] text-text placeholder:text-text-tertiary"
          />
          <button
            onClick={(e) => {
              handlePress(e);
              if (apiKey.trim()) {
                onSaveApiKey(apiKey.trim());
                setApiKey("");
              }
            }}
            className="px-4 py-2.5 bg-text text-bg rounded-md text-xs font-semibold shrink-0"
          >
            Save Key
          </button>
        </div>
        <p className="text-[11px] text-text-tertiary mt-2 leading-relaxed">
          {isLocalEngine
            ? "Only needed for the GPT text-cleanup step. With the local Parakeet engine selected, transcription itself works without any key."
            : "Required for transcription and text cleanup."}
        </p>
      </div>
    );
  }

  // Update mode — blank input with Save Key + Cancel
  if (view === "update") {
    return (
      <div>
        {enginePicker}
        <div className="text-xs font-semibold text-text-secondary mb-2">
          OpenAI API Key
        </div>
        <div className="flex items-center gap-2.5">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-proj-..."
            className="flex-1 px-3.5 py-2.5 bg-surface border border-border rounded-sm text-[13px] text-text placeholder:text-text-tertiary"
            autoFocus
          />
          <button
            onClick={(e) => {
              handlePress(e);
              if (apiKey.trim()) {
                onSaveApiKey(apiKey.trim());
                setApiKey("");
              }
            }}
            className="px-4 py-2.5 bg-text text-bg rounded-md text-xs font-semibold shrink-0"
          >
            Save Key
          </button>
        </div>
        <div className="mt-3 flex gap-2">
          <button
            onClick={(e) => {
              handlePress(e);
              setApiKey("");
              setView("display");
            }}
            className="px-4 py-1.5 bg-surface-elevated border border-border-strong rounded-md text-xs font-semibold text-text"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  // Configured — read-only input with masked key + status badge, buttons below
  return (
    <div>
      {enginePicker}
      <div className="text-xs font-semibold text-text-secondary mb-2">
        OpenAI API Key
      </div>
      <div className="flex items-center gap-2.5">
        <input
          type="text"
          value={apiKeyHint}
          readOnly
          className="flex-1 px-3.5 py-2.5 bg-surface border border-border rounded-sm text-[13px] text-text"
        />
        <div className="inline-flex items-center gap-1.5 text-[11px] font-semibold text-accent bg-accent-bg border border-accent-border px-3 py-1 rounded-xs shrink-0">
          <div className="w-[5px] h-[5px] rounded-full bg-accent" />
          Configured
        </div>
      </div>
      <div className="mt-3 flex gap-2">
        <button
          onClick={(e) => {
            handlePress(e);
            setView("update");
          }}
          className="px-4 py-1.5 bg-surface-elevated border border-border-strong rounded-md text-xs font-semibold text-text"
        >
          Update Key
        </button>
        <button
          onClick={(e) => {
            handlePress(e);
            setShowRemoveConfirm(true);
          }}
          className="px-4 py-1.5 bg-transparent rounded-md text-xs font-semibold text-danger"
        >
          Remove
        </button>
      </div>

      {showRemoveConfirm && (
        <div className="mt-4 p-3 bg-surface border border-border-strong rounded-sm">
          <p className="text-sm text-text-secondary mb-3">
            Are you sure? Dictto won't work without an API key.
          </p>
          <div className="flex gap-2">
            <button
              onClick={async (e) => {
                handlePress(e);
                await onRemoveApiKey();
                setShowRemoveConfirm(false);
              }}
              className="px-4 py-1.5 bg-danger text-white rounded-md text-xs font-semibold"
            >
              Yes, remove
            </button>
            <button
              onClick={(e) => {
                handlePress(e);
                setShowRemoveConfirm(false);
              }}
              className="px-4 py-1.5 bg-surface-elevated border border-border-strong rounded-md text-xs font-semibold text-text"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
