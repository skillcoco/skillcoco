// Phase 6 (Certification) — Plan 06-04 (Wave 3) AchievementCard.
//
// One row per Achievement: level icon (lucide) + "<Level> — <Track>",
// formatted issued date, and a kind-aware Export button (PDF for the
// completion certificate, PNG for any badge). Glassmorphism palette
// matches the rest of the Dashboard (white/10 borders + bg-white/5 +
// backdrop-blur). D-08: typography-driven; D-10: no emojis anywhere.

import { Award, BadgeCheck, Trophy, Download } from "lucide-react";
import type { Achievement, AchievementLevel } from "@/types/achievements";
import { useAchievementsStore } from "@/stores/useAchievementsStore";

function iconForLevel(level: AchievementLevel) {
  if (level === "Associate") return Award;
  if (level === "Practitioner") return BadgeCheck;
  // Professional + Completion
  return Trophy;
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso.slice(0, 10);
  }
}

interface Props {
  achievement: Achievement;
}

export function AchievementCard({ achievement }: Props) {
  // Sibling-slice selectors — Wave 1 store actions (sibling NOT
  // useLearningStore). Two distinct selectors so a single export click
  // doesn't subscribe to the entire store surface.
  const exportCertificate = useAchievementsStore((s) => s.exportCertificate);
  const exportBadge = useAchievementsStore((s) => s.exportBadge);
  const Icon = iconForLevel(achievement.level);

  const onExport = async () => {
    if (achievement.kind === "certificate") {
      await exportCertificate(achievement);
    } else {
      await exportBadge(achievement);
    }
  };

  const kindLabel =
    achievement.kind === "certificate" ? "Completion Certificate" : "Badge";

  return (
    <div
      data-testid={`achievement-card-${achievement.id}`}
      className="flex items-center gap-3 rounded-lg border border-white/10 bg-white/5 p-4 backdrop-blur"
    >
      <Icon className="h-8 w-8 text-amber-300" aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold text-foreground">
          {achievement.level} — {achievement.trackTopic}
        </div>
        <div className="mt-0.5 text-xs text-muted-foreground">
          {formatDate(achievement.issuedAt)} · {kindLabel}
        </div>
      </div>
      <button
        type="button"
        onClick={onExport}
        aria-label={`Export ${achievement.level} ${achievement.kind}`}
        className="inline-flex items-center gap-1 rounded-md bg-white/10 px-2.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-white/20"
      >
        <Download className="h-3 w-3" aria-hidden />
        Export
      </button>
    </div>
  );
}
