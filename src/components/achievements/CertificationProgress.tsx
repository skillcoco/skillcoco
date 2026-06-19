// Phase 08.2 (Cert Simplification + Gamification) — TrackView progress.
//
// Replaces the Phase 6 3-row indicator (Associate / Practitioner /
// Professional) with the new model (D-20):
//   - 4-segment progress bar (0-25, 25-50, 50-75, 75-100)
//   - Milestone markers at 25/50/75 (earned / locked states)
//   - Completion certificate badge at 100% with Download PDF button
//
// Reads `getTrackCertifications(trackId)` on mount AND inspects the
// achievements store for Milestone25/50/75/Completion rows on this
// track. The component subscribes to the store so a NEW milestone or
// completion that arrives via submit_quiz's newlyIssuedAchievements
// path triggers an automatic re-render.
//
// No emojis. Lucide icons only. Graceful IPC-error fallback (TrackView
// must not crash).

import { useEffect, useMemo, useRef, useState } from "react";
import { BadgeCheck, Lock, Trophy } from "lucide-react";
import { getTrackCertifications } from "@/lib/tauri-commands";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import type { Achievement, TrackCertifications } from "@/types/achievements";
import {
  MILESTONE_LEVELS,
  milestoneThreshold,
  type MilestoneLevel,
} from "@/types/achievements";

interface Props {
  trackId: string;
}

interface MilestoneRow {
  level: MilestoneLevel;
  threshold: number;
  earned: boolean;
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

  // Re-fetch on new achievement arrival for this track.
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

  // Compute milestone earned state from the store achievements (the
  // store is the source of truth; we don't need TrackCertifications
  // for milestones since those are the new model).
  const trackAchievements = useMemo<Achievement[]>(
    () => achievements.filter((a) => a.trackId === trackId),
    [achievements, trackId],
  );

  const milestones = useMemo<MilestoneRow[]>(
    () =>
      MILESTONE_LEVELS.map((level) => ({
        level,
        threshold: milestoneThreshold(level),
        earned: trackAchievements.some((a) => a.level === level),
      })),
    [trackAchievements],
  );

  const completionAchievement = trackAchievements.find(
    (a) => a.kind === "certificate" && a.level === "Completion",
  );
  const completionEarned = completionAchievement !== undefined;

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

  // Approximate progress percent for the bar. We do not yet have the
  // raw `modules_mastered / modules_total` exposed via IPC — the
  // existing TrackCertifications shape only tells us which levels were
  // earned. For the bar fill we use the count of earned milestones as
  // a coarse proxy: 0/3 = 0%, 1/3 = 33%, etc., capped at 100% on
  // completion. This is the same proxy used by the previous Phase 6
  // 3-tier renderer and is good enough for the visual cue.
  const earnedCount = milestones.filter((m) => m.earned).length;
  const fillPercent = completionEarned
    ? 100
    : earnedCount === 0
      ? 5
      : milestones[earnedCount - 1].threshold;

  return (
    <section
      data-testid="certification-progress"
      aria-label="Certification progress"
      className="glass space-y-3 rounded-md border border-border p-3"
    >
      <header className="flex items-center gap-2 text-xs uppercase tracking-wider text-muted-foreground">
        <Trophy className="h-3 w-3" aria-hidden />
        Certification Progress
      </header>

      {/* 4-segment progress bar */}
      <div
        data-testid="cert-progress-bar"
        className="relative h-2 w-full rounded-full bg-white/10"
      >
        <div
          data-testid="cert-progress-bar-fill"
          className={`absolute left-0 top-0 h-2 rounded-full ${
            completionEarned ? "bg-emerald-500" : "bg-amber-400"
          }`}
          style={{ width: `${fillPercent}%` }}
          aria-label={`${fillPercent}% complete`}
        />
        {/* Tick markers at 25/50/75 — absolute positioned over the bar. */}
        {[25, 50, 75].map((tick) => (
          <span
            key={tick}
            data-testid={`cert-progress-tick-${tick}`}
            className="absolute top-0 h-2 w-px bg-background"
            style={{ left: `${tick}%` }}
            aria-hidden
          />
        ))}
      </div>

      {/* Milestone marker row */}
      <ul className="flex justify-between gap-2 text-xs">
        {milestones.map((m) => {
          const Icon = m.earned ? BadgeCheck : Lock;
          return (
            <li
              key={m.level}
              data-testid={`milestone-row-${m.level}`}
              className="flex items-center gap-1"
            >
              <Icon
                data-testid={`milestone-icon-${m.level}-${
                  m.earned ? "earned" : "locked"
                }`}
                className={`h-3 w-3 ${
                  m.earned ? "text-emerald-400" : "text-muted-foreground/50"
                }`}
                aria-hidden
              />
              <span
                className={
                  m.earned ? "text-foreground" : "text-muted-foreground/60"
                }
              >
                {m.threshold}%
              </span>
            </li>
          );
        })}
        <li
          data-testid="milestone-row-Completion"
          className="flex items-center gap-1"
        >
          <Trophy
            data-testid={`milestone-icon-Completion-${
              completionEarned ? "earned" : "locked"
            }`}
            className={`h-3 w-3 ${
              completionEarned
                ? "text-amber-400"
                : "text-muted-foreground/50"
            }`}
            aria-hidden
          />
          <span
            className={
              completionEarned
                ? "text-foreground"
                : "text-muted-foreground/60"
            }
          >
            100%
          </span>
        </li>
      </ul>

      {/* Completion certificate row — shown once 100% reached. */}
      {completionEarned && completionAchievement && (
        <div className="flex items-center justify-between border-t border-border pt-2">
          <span className="text-xs text-foreground">
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
