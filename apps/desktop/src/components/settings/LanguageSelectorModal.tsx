import { useState, useEffect, useMemo } from "react";
import { LANGUAGES, Language } from "../../data/languages";
import { FlagIcon } from "./FlagIcon";

interface LanguageSelectorModalProps {
  isOpen: boolean;
  initialLanguages: string[];  // e.g. ["es", "en"] or ["auto"]
  onSave: (languages: string[]) => void;
  onClose: () => void;
}

export function LanguageSelectorModal({
  isOpen,
  initialLanguages,
  onSave,
  onClose,
}: LanguageSelectorModalProps) {
  const [draft, setDraft] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const [autoDetect, setAutoDetect] = useState(false);
  const [previousDraft, setPreviousDraft] = useState<string[]>([]);

  // Initialize draft state when modal opens
  useEffect(() => {
    if (isOpen) {
      if (initialLanguages.length === 1 && initialLanguages[0] === "auto") {
        setAutoDetect(true);
        setDraft([]);
      } else {
        setAutoDetect(false);
        setDraft([...initialLanguages]);
      }
      setSearch("");
    }
  }, [isOpen]);

  const filteredLanguages = useMemo(() => {
    const q = search.toLowerCase().trim();
    if (!q) return LANGUAGES;
    const pinned = LANGUAGES.filter((l) => l.pinned && l.name.toLowerCase().includes(q));
    const rest = LANGUAGES.filter((l) => !l.pinned && l.name.toLowerCase().includes(q));
    return [...pinned, ...rest];
  }, [search]);

  const selectedLanguages = useMemo(() => {
    return draft
      .map((code) => LANGUAGES.find((l) => l.code === code))
      .filter((l): l is Language => l !== undefined);
  }, [draft]);

  const handleAutoDetectToggle = () => {
    if (!autoDetect) {
      // Turning ON: back up current draft, clear grid
      if (draft.length > 0) {
        setPreviousDraft(draft);
      }
      setAutoDetect(true);
    } else {
      // Turning OFF: restore previous draft if available
      setDraft(previousDraft.length > 0 ? previousDraft : draft);
      setAutoDetect(false);
    }
  };

  const handleCardPress = (
    e: React.MouseEvent<HTMLDivElement>,
    code: string
  ) => {
    if (autoDetect) return;
    const el = e.currentTarget;
    el.classList.remove("btn-press");
    void el.offsetWidth;
    el.classList.add("btn-press");
    setDraft((prev) =>
      prev.includes(code) ? prev.filter((c) => c !== code) : [...prev, code]
    );
  };

  const handleSave = (e: React.MouseEvent<HTMLButtonElement>) => {
    const btn = e.currentTarget;
    btn.classList.remove("btn-press");
    void btn.offsetWidth;
    btn.classList.add("btn-press");
    onSave(autoDetect ? ["auto"] : draft);
    onClose();
  };

  const isDisabled = !autoDetect && draft.length === 0;

  if (!isOpen) return null;

  return (
    // Backdrop
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(18, 17, 15, 0.75)" }}
    >
      {/* Modal card */}
      <div
        className="flex flex-col bg-bg border border-border-strong rounded-lg"
        style={{
          width: "680px",
          maxWidth: "calc(100vw - 40px)",
          height: "520px",
          maxHeight: "calc(100vh - 40px)",
        }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-border shrink-0">
          <div>
            <h2 className="text-base font-bold text-text">Languages</h2>
            <p className="text-xs text-text-tertiary mt-0.5">
              Select the languages you want to use with Dictto
            </p>
          </div>
          {/* Auto-detect toggle */}
          <div className="flex items-center gap-3">
            <span className="text-xs text-text-secondary font-medium">Auto-detect</span>
            <button
              onClick={handleAutoDetectToggle}
              className={`relative w-10 h-5 rounded-full transition-colors ${
                autoDetect ? "bg-accent" : "bg-surface-elevated"
              }`}
              aria-label="Auto-detect toggle"
            >
              <span
                className={`absolute top-0.5 left-0.5 w-4 h-4 bg-text rounded-full transition-transform ${
                  autoDetect ? "translate-x-5" : ""
                }`}
              />
            </button>
          </div>
        </div>

        {/* Search box */}
        <div className="px-4 pt-3 pb-2 shrink-0">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search for any language..."
            className="w-full bg-surface border border-border rounded-sm text-sm text-text px-3 py-2 outline-none placeholder:text-text-tertiary"
            style={
              autoDetect
                ? { opacity: 0.3, pointerEvents: "none" }
                : undefined
            }
          />
        </div>

        {/* Content: grid + sidebar */}
        <div
          className="flex flex-1 overflow-hidden"
          style={autoDetect ? { opacity: 0.3, pointerEvents: "none" } : undefined}
        >
          {/* Language grid */}
          <div className="flex-[2] overflow-y-auto p-3">
            <div className="grid grid-cols-3 gap-2">
              {filteredLanguages.map((lang) => {
                const isSelected = draft.includes(lang.code);
                return (
                  <div
                    key={lang.code}
                    onClick={(e) => handleCardPress(e, lang.code)}
                    className={`flex items-center gap-2 px-3 py-2.5 rounded-sm cursor-pointer select-none border ${
                      isSelected
                        ? "bg-surface-elevated border-border-strong"
                        : "bg-surface border-border"
                    }`}
                  >
                    <FlagIcon countryCode={lang.countryCode} className="w-5 h-3.5 shrink-0 rounded-[2px]" />
                    <span className="text-xs text-text truncate">{lang.name}</span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Sidebar */}
          <div className="flex-[1] border-l border-border overflow-y-auto p-4">
            <p className="text-xs font-semibold text-text-secondary uppercase tracking-wide mb-3">
              Selected
            </p>
            {selectedLanguages.length === 0 ? (
              <p className="text-xs text-text-tertiary">No languages selected</p>
            ) : (
              <div className="space-y-2">
                {selectedLanguages.map((lang) => (
                  <div key={lang.code} className="flex items-center gap-2">
                    <FlagIcon countryCode={lang.countryCode} className="w-4 h-3 shrink-0 rounded-[2px]" />
                    <span className="text-xs text-text">{lang.name}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-3 border-t border-border shrink-0">
          {isDisabled ? (
            <p className="text-xs text-text-tertiary">Select at least one language</p>
          ) : (
            <span />
          )}
          <button
            onClick={handleSave}
            disabled={isDisabled}
            className={`px-4 py-2 bg-surface-elevated border border-border-strong rounded-sm text-sm font-medium text-text ${
              isDisabled ? "opacity-50 cursor-not-allowed" : ""
            }`}
          >
            Save and close
          </button>
        </div>
      </div>
    </div>
  );
}
