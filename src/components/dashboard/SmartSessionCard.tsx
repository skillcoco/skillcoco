import { Link } from "react-router-dom";
import { Zap, ArrowRight } from "lucide-react";

interface SmartSessionCardProps {
  dueCount: number;
  nextModuleName: string | null;
  estimatedMinutes: number;
}

export function SmartSessionCard({ dueCount, nextModuleName, estimatedMinutes }: SmartSessionCardProps) {
  const hasModule = nextModuleName !== null;

  // Build recommendation text
  const parts: string[] = [];
  if (dueCount > 0 && hasModule) {
    const half = Math.ceil(dueCount / 2);
    const rest = dueCount - half;
    parts.push(`${half} review cards`);
    parts.push(`Continue "${nextModuleName}"`);
    if (rest > 0) parts.push(`${rest} review cards`);
  } else if (dueCount > 0) {
    parts.push(`${dueCount} review cards`);
  } else if (hasModule) {
    parts.push(`Continue "${nextModuleName}"`);
  }

  const recommendation = parts.join("  ->  ");

  return (
    <div className="relative overflow-hidden rounded-xl p-[2px]">
      {/* Gradient border */}
      <div className="absolute inset-0 rounded-xl bg-gradient-to-r from-accent via-accent to-primary" />

      {/* Card interior */}
      <div className="relative flex items-center justify-between gap-4 rounded-[10px] bg-[hsl(var(--card))] px-6 py-5">
        <div className="flex items-center gap-4">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-accent to-primary">
            <Zap size={20} className="text-primary-foreground" />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-foreground">Smart Session</h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Recommended: {recommendation}.{" "}
              Estimated {estimatedMinutes} minutes.
            </p>
          </div>
        </div>

        <Link
          to="/review"
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-gradient-to-r from-accent to-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90"
        >
          Start Session
          <ArrowRight size={16} />
        </Link>
      </div>
    </div>
  );
}
