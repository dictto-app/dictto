import { ComingSoon } from "./ComingSoon";
import { LANGUAGES, Language } from "../../data/languages";
import { FlagIcon } from "./FlagIcon";

function LanguageInlineDisplay({ codes }: { codes: string[] }) {
  if (codes.length === 1 && codes[0] === "auto") {
    return <span className="text-xs text-text-secondary">Auto-detect</span>;
  }
  const found = codes
    .map((code) => LANGUAGES.find((l) => l.code === code))
    .filter((l): l is Language => l !== undefined);
  if (found.length === 0) return null;

  const MAX_INLINE = 2;
  const visible = found.slice(0, MAX_INLINE);
  const remainder = found.length - MAX_INLINE;

  return (
    <span className="flex items-center gap-1 text-xs text-text-secondary">
      {visible.map((l, i) => (
        <span key={l.code} className="flex items-center gap-1">
          {i > 0 && <span className="mx-0.5">&middot;</span>}
          <FlagIcon countryCode={l.countryCode} className="w-4 h-3 rounded-[2px]" />
          {l.name}
        </span>
      ))}
      {remainder > 0 && <span className="ml-0.5">&middot; +{remainder} more</span>}
    </span>
  );
}

interface Props {
  settings: Record<string, string>;
  onSave: (key: string, value: string) => void;
  onOpenLanguageModal: () => void;
}

export function GeneralTab({ settings, onSave, onOpenLanguageModal }: Props) {
  return (
    <div className="space-y-6">
      {/* Start on boot — functional */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">Start on boot</p>
          <p className="text-xs text-text-tertiary">Launch Dictto when you sign in</p>
        </div>
        <button
          onClick={() =>
            onSave("auto_start", settings.auto_start === "true" ? "false" : "true")
          }
          className={`relative w-10 h-5 rounded-full transition-colors ${
            settings.auto_start === "true" ? "bg-accent" : "bg-surface-elevated"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 w-4 h-4 bg-text rounded-full transition-transform ${
              settings.auto_start === "true" ? "translate-x-5" : ""
            }`}
          />
        </button>
      </div>

      {/* Sound effects — functional */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">Sound effects</p>
          <p className="text-xs text-text-tertiary">Play sounds on recording events</p>
        </div>
        <button
          onClick={() =>
            onSave("sound_effects_enabled", settings.sound_effects_enabled === "true" ? "false" : "true")
          }
          className={`relative w-10 h-5 rounded-full transition-colors ${
            settings.sound_effects_enabled === "true" ? "bg-accent" : "bg-surface-elevated"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 w-4 h-4 bg-text rounded-full transition-transform ${
              settings.sound_effects_enabled === "true" ? "translate-x-5" : ""
            }`}
          />
        </button>
      </div>

      {/* Languages — functional */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">Languages</p>
          <p className="text-xs text-text-tertiary">The languages you speak</p>
        </div>
        <div className="flex items-center gap-3">
          <LanguageInlineDisplay codes={(() => {
            try { return JSON.parse(settings.languages || "[]"); }
            catch { return []; }
          })()} />
          <button
            onClick={(e) => {
              const btn = e.currentTarget;
              btn.classList.remove("btn-press");
              void btn.offsetWidth;
              btn.classList.add("btn-press");
              onOpenLanguageModal();
            }}
            className="px-3 py-1.5 bg-surface-elevated border border-border-strong rounded-sm text-xs font-medium text-text-secondary"
          >
            Change
          </button>
        </div>
      </div>

      {/* Push-to-talk hotkey — Coming Soon */}
      <ComingSoon>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-text-secondary">Push-to-talk hotkey</p>
            <p className="text-xs text-text-tertiary">Currently: Ctrl+Win</p>
          </div>
          <button className="px-3 py-1.5 bg-surface-elevated border border-border-strong rounded-sm text-xs font-medium text-text-secondary">
            Change
          </button>
        </div>
      </ComingSoon>
    </div>
  );
}
