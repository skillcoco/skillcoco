// Phase 6 (Certification) — Plan 06-05 (Wave 4) TrackView progress.
//
// Per D-11 + CERT-11: three-row indicator (Associate / Practitioner /
// Professional). Reads getTrackCertifications(trackId) on mount, derives
// earned vs in-progress vs locked row state from the response, and shows
// criteria text for the next-tier row only. When Professional has been
// earned AND a Completion certificate achievement exists in the store,
// a Download PDF link is exposed (calls store.exportCertificate). The
// component is glass-styled, lucide-icons-only (no emoji per D-08), and
// gracefully degrades to a small error note on IPC failure so it does
// NOT crash the surrounding TrackView (T-06-17 mitigation).
//
// Re-fetch trigger: the component subscribes to useAchievementsStore's
// `achievements` slice. When a NEW achievement for this trackId arrives
// in the store (typically via Wave 3's submitQuiz → appendNewlyIssued
// path), the component re-calls getTrackCertifications so earned-levels
// + next-level state stays in sync with what the backend just issued.

import { useEffect, useRef, useState } from "react";
import { BadgeCheck, Hourglass, Lock, Trophy } from "lucide-react";
import { getTrackCertifications } from "@/lib/tauri-commands";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import type {
  AchievementLevel,
  TrackCertifications,
} from "@/types/achievements";

const LEVELS: ReadonlyArray<Extract<
  AchievementLevel,
  "Associate" | "Practitioner" | "Professional"
>> = ["Associate", "Practitioner", "Professional"];

interface Props {
  trackId: string;
}

export function CertificationProgress({ trackId }: Props) {
  const [data, setData] = useState<TrackCertifications | null>(null);
  const [error, setError] = useState<string | null>(null);
  const achievements = useAchievementsStore((s) => s.achievements);
  const exportCertificate = useAchievementsStore((s) => s.exportCertificate);

  // Track the count of relevant-to-this-track achievements so a state
  // change to an unrelated track does not trigger a needless re-fetch.
  const relevantCountRef = useRef(0);

  // Initial load.
  useEffect(() => {
    let cancelled = false;
    setError(null);
    getTrackCertifications({ trackId })
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [trackId]);

  // Re-fetch on new achievement arrival for this track. We guard by
  // counting trackId-matching achievements so unrelated store updates
  // (e.g. another track's badge) do NOT re-hit the IPC.
  useEffect(() => {
    const count = achievements.filter((a) => a.trackId === trackId).length;
    if (count <= relevantCountRef.current) {
      relevantCountRef.current = count;
      return;
    }
    relevantCountRef.current = count;
    getTrackCertifications({ trackId })
      .then(setData)
      .catch(() => {
        // Silent: initial-load error is already surfaced; a re-fetch
        // failure should not flap the UI.
      });
  }, [achievements, trackId]);

  if (error) {
    return (
      <div
        data-testid="cert-progress-error"
        className="text-xs italic text-muted-foreground"
      >
        Could not load certifications
      </div>
    );
  }

  if (!data) {
    return (
      <div
        data-testid="cert-progress-loading"
        className="text-xs text-muted-foreground"
      >
        Loading certifications…
      </div>
    );
  }

  const earned = new Set<AchievementLevel>(data.earnedLevels);
  const next = data.nextLevel;
  const completionEarned = earned.has("Professional");
  const completionAchievement = achievements.find(
    (a) => a.trackId === trackId && a.kind === "certificate" && a.level === "Completion",
  );

  return (
    <section
      data-testid="certification-progress"
      aria-label="Certification progress"
      className="glass space-y-2 rounded-md border border-border p-3"
    >
      <header className="flex items-center gap-2 text-xs uppercase tracking-wider text-muted-foreground">
        <Trophy className="h-3 w-3" aria-hidden />
        Certifications
      </header>
      <ul className="space-y-1">
        {LEVELS.map((lvl) => {
          const isEarned = earned.has(lvl);
          const isNext = lvl === next;
          const Icon = isEarned ? BadgeCheck : isNext ? Hourglass : Lock;
          const variant = isEarned ? "check" : isNext ? "progress" : "lock";
          return (
            <li
              key={lvl}
              data-testid={`cert-row-${lvl}`}
              className="flex items-start gap-2 text-sm"
            >
              <Icon
                data-testid={`cert-row-${lvl}-icon-${variant}`}
                className={`mt-0.5 h-4 w-4 ${
                  isEarned
                    ? "text-emerald-400"
                    : isNext
                    ? "text-amber-400"
                    : "text-muted-foreground/50"
                }`}
                aria-hidden
              />
              <div className="min-w-0 flex-1">
                <div
                  className={
                    isEarned
                      ? "text-foreground"
                      : isNext
                      ? "text-foreground"
                      : "text-muted-foreground/60"
                  }
                >
                  {lvl}
                </div>
                {isNext && data.criteria && (
                  <div className="text-xs text-muted-foreground">{data.criteria}</div>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {completionEarned && completionAchievement && (
        <div className="flex items-center justify-between border-t border-border pt-2">
          <span className="text-xs text-muted-foreground">
            Completion certificate earned
          </span>
          <button
            type="button"
            onClick={() => exportCertificate(completionAchievement)}
            className="text-xs text-primary hover:underline"
          >
            Download PDF
          </button>
        </div>
      )}
    </section>
  );
}
