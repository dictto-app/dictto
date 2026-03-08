import * as Flags from "country-flag-icons/react/3x2";

type FlagCode = keyof typeof Flags;

interface FlagIconProps {
  countryCode: string | null;
  className?: string;
}

/**
 * Renders an SVG country flag from country-flag-icons, or a globe fallback
 * for languages without a clear national flag (Basque, Latin, etc.).
 */
export function FlagIcon({ countryCode, className = "w-5 h-3.5" }: FlagIconProps) {
  if (!countryCode) {
    return (
      <svg className={className} viewBox="0 0 24 16" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect width="24" height="16" rx="2" fill="currentColor" opacity="0.15" />
        <circle cx="12" cy="8" r="5" stroke="currentColor" strokeWidth="1.2" opacity="0.4" fill="none" />
        <line x1="12" y1="3" x2="12" y2="13" stroke="currentColor" strokeWidth="1" opacity="0.3" />
        <line x1="7" y1="8" x2="17" y2="8" stroke="currentColor" strokeWidth="1" opacity="0.3" />
      </svg>
    );
  }

  const Flag = Flags[countryCode as FlagCode];
  if (!Flag) {
    return (
      <svg className={className} viewBox="0 0 24 16" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect width="24" height="16" rx="2" fill="currentColor" opacity="0.15" />
        <circle cx="12" cy="8" r="5" stroke="currentColor" strokeWidth="1.2" opacity="0.4" fill="none" />
        <line x1="12" y1="3" x2="12" y2="13" stroke="currentColor" strokeWidth="1" opacity="0.3" />
        <line x1="7" y1="8" x2="17" y2="8" stroke="currentColor" strokeWidth="1" opacity="0.3" />
      </svg>
    );
  }

  return <Flag className={className} />;
}
