interface StatsCardProps {
  label: string;
  value: string | number;
  subtitle: string;
  icon: React.ReactNode;
  accentColor?: string;
}

export function StatsCard({ label, value, subtitle, icon, accentColor }: StatsCardProps) {
  return (
    <div className="glass rounded-xl p-5 flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground">{label}</span>
        <span
          className="flex h-8 w-8 items-center justify-center rounded-lg bg-white/5"
          style={accentColor ? { color: accentColor } : undefined}
        >
          {icon}
        </span>
      </div>
      <div
        className="text-3xl font-bold"
        style={accentColor ? { color: accentColor } : undefined}
      >
        {value}
      </div>
      <span className="text-xs text-muted-foreground">{subtitle}</span>
    </div>
  );
}
