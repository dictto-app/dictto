import { ComingSoon } from "./ComingSoon";
import { LANGUAGES, Language } from "../../data/languages";
import { FlagIcon } from "./FlagIcon";
import { localeOptions, useT } from "../../i18n";

function LanguageInlineDisplay({ codes }: { codes: string[] }) {
  const t = useT();
  if (codes.length === 1 && codes[0] === "auto") {
    return <span className="text-xs text-text-secondary">{t("autoDetect")}</span>;
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
      {remainder > 0 && (
        <span className="ml-0.5">&middot; {t("moreLanguages", { n: remainder })}</span>
      )}
    </span>
  );
}

const selectClass =
  "px-3 py-1.5 bg-surface-elevated border border-border-strong rounded-sm text-xs font-medium text-text-secondary";

interface Props {
  settings: Record<string, string>;
  onSave: (key: string, value: string) => void;
  onOpenLanguageModal: () => void;
}

export function GeneralTab({ settings, onSave, onOpenLanguageModal }: Props) {
  const t = useT();

  return (
    <div className="space-y-6">
      {/* Start on boot — functional */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">{t("startOnBoot")}</p>
          <p className="text-xs text-text-tertiary">{t("startOnBootDesc")}</p>
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
          <p className="text-sm font-medium text-text-secondary">{t("soundEffects")}</p>
          <p className="text-xs text-text-tertiary">{t("soundEffectsDesc")}</p>
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
          <p className="text-sm font-medium text-text-secondary">{t("languages")}</p>
          <p className="text-xs text-text-tertiary">{t("languagesDesc")}</p>
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
            {t("change")}
          </button>
        </div>
      </div>

      {/* Interface language */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-text-secondary">{t("interfaceLanguage")}</p>
          <p className="text-xs text-text-tertiary">{t("interfaceLanguageDesc")}</p>
        </div>
        <select
          value={settings.ui_locale || "en"}
          onChange={(e) => onSave("ui_locale", e.target.value)}
          className={selectClass}
          style={{ colorScheme: "dark" }}
        >
          {localeOptions.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.nativeLabel}
            </option>
          ))}
        </select>
      </div>

      {/* Push-to-talk hotkey — Coming Soon */}
      <ComingSoon>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-text-secondary">{t("pushToTalkHotkey")}</p>
            <p className="text-xs text-text-tertiary">{t("hotkeyCurrently", { hotkey: "Ctrl+Win" })}</p>
          </div>
          <button className="px-3 py-1.5 bg-surface-elevated border border-border-strong rounded-sm text-xs font-medium text-text-secondary">
            {t("change")}
          </button>
        </div>
      </ComingSoon>
    </div>
  );
}
