interface ComingSoonProps {
  children: React.ReactNode;
}

export function ComingSoon({ children }: ComingSoonProps) {
  return (
    <div className="opacity-30 pointer-events-none select-none">
      {children}
      <p className="text-xs text-text-tertiary mt-1">Coming soon</p>
    </div>
  );
}
