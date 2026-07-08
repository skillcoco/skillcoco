// Phase 08.2 (Cert Simplification + Gamification) — AchievementCard.
// Phase 13-03 (OSS Consolidation) — badge PNG export CTA added per D-10.
//
// Visual variants per kind (D-21):
//   - kind="certificate" → large card with Download PDF CTA + Download badge PNG CTA
//   - kind="badge" → compact pill / chip (Milestone25/50/75 + legacy badges)
//
// Legacy 3-tier rows (Associate / Practitioner / Professional) from
// pre-08.2 testing data still render — they fall into the badge variant
// with their original level text (D-02 — keep as-is, "old logbook entry").
// D-10: these legacy rows + cert rows get the badge PNG export CTA.
//
// Lucide icons + plain text. No emojis. Badge PNG export is gated:
//   - cert kind: always eligible (canExportBadge = true)
//   - badge kind: Associate/Practitioner/Professional eligible
//   - Milestone25/50/75: in-app only per Phase 08.2 D-05 (reaffirmed D-10)

import { Award, BadgeCheck, Download, Trophy } from "lucide-react";
import type { Achievement, AchievementLevel } from "@/types/achievements";
import { useAchievementsStore } from "@/stores/useAchievementsStore";

function iconForLevel(level: AchievementLevel) {
  switch (level) {
    case "Completion":
      return Trophy;
    case "Milestone25":
    case "Milestone50":
    case "Milestone75":
      return BadgeCheck;
    case "Associate":
      return Award;
    case "Practitioner":
      return BadgeCheck;
    case "Professional":
      return Trophy;
  }
}

function readableLevel(level: AchievementLevel): string {
  switch (level) {
    case "Milestone25":
      return "25% Milestone";
    case "Milestone50":
      return "50% Milestone";
    case "Milestone75":
      return "75% Milestone";
    default:
      return level;
  }
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
  const exportCertificate = useAchievementsStore((s) => s.exportCertificate);
  const exportBadge = useAchievementsStore((s) => s.exportBadge);
  const Icon = iconForLevel(achievement.level);
  const isCert = achievement.kind === "certificate";

  // D-10: badge PNG export is available for cert-kind rows AND legacy
  // Associate/Practitioner/Professional badge rows. Milestone chips are
  // in-app only (Phase 08.2 D-05, reaffirmed D-10).
  const canExportBadge =
    isCert ||
    ["Associate", "Practitioner", "Professional"].includes(achievement.level);

  const onExportCert = async () => {
    await exportCertificate(achievement);
  };

  const onExportBadge = async () => {
    await exportBadge(achievement);
  };

  // ── Certificate variant — large card with Download CTA ────────────
  if (isCert) {
    return (
      <div
        data-testid={`achievement-card-${achievement.id}`}
        data-variant="certificate"
        className="flex items-center gap-3 rounded-lg border border-amber-300/30 bg-amber-300/5 p-4 backdrop-blur"
      >
        <Icon className="h-10 w-10 text-amber-300" aria-hidden />
        <div className="min-w-0 flex-1">
          <div className="truncate text-base font-semibold text-foreground">
            {readableLevel(achievement.level)} — {achievement.trackTopic}
          </div>
          <div className="mt-0.5 text-xs text-muted-foreground">
            {formatDate(achievement.issuedAt)} · Completion Certificate
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onExportCert}
            aria-label={`Download ${achievement.level} certificate PDF`}
            className="inline-flex items-center gap-1 rounded-md bg-amber-300/20 px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-amber-300/30"
          >
            <Download className="h-3 w-3" aria-hidden />
            Download PDF
          </button>
          {canExportBadge && (
            <button
              type="button"
              onClick={onExportBadge}
              aria-label={`Download ${achievement.level} badge PNG`}
              className="inline-flex items-center gap-1 rounded-md bg-amber-300/20 px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-amber-300/30"
            >
              <Download className="h-3 w-3" aria-hidden />
              Download badge PNG
            </button>
          )}
        </div>
      </div>
    );
  }

  // ── Badge / milestone variant — compact pill ──────────────────────
  return (
    <div
      data-testid={`achievement-card-${achievement.id}`}
      data-variant="badge"
      className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1.5 backdrop-blur"
    >
      <Icon className="h-4 w-4 text-emerald-400" aria-hidden />
      <span className="text-xs font-medium text-foreground">
        {readableLevel(achievement.level)}
      </span>
      <span className="text-xs text-muted-foreground">
        · {achievement.trackTopic}
      </span>
      {canExportBadge && (
        <button
          type="button"
          onClick={onExportBadge}
          aria-label={`Download ${achievement.level} badge PNG`}
          className="inline-flex items-center gap-1 rounded-md bg-white/10 px-2 py-1 text-xs font-medium text-foreground transition-colors hover:bg-white/20"
        >
          <Download className="h-3 w-3" aria-hidden />
          Download badge PNG
        </button>
      )}
    </div>
  );
}
