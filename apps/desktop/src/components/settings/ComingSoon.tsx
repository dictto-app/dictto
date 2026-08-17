import { useT } from "../../i18n";

interface ComingSoonProps {
  children: React.ReactNode;
}

export function ComingSoon({ children }: ComingSoonProps) {
  const t = useT();
  return (
    <div className="opacity-30 pointer-events-none select-none">
      {children}
      <p className="text-xs text-text-tertiary mt-1">{t("comingSoon")}</p>
    </div>
  );
}
