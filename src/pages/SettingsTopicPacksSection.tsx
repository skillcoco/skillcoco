// Phase 5 Plan 04 (Wave 3) — Settings → "Topic Packs" section.
// Visual sibling of SettingsLabsSection.tsx. Source badge per row = R1
// attribution. JSX-only text rendering = T-05-13 mitigation (never
// dangerouslySetInnerHTML). Disabled packs render at reduced opacity.

import { useEffect, useState } from "react";
import {
  Loader2,
  RefreshCw,
  Package2,
  AlertTriangle,
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useTopicPacksStore } from "@/stores/useTopicPacksStore";
import type { TopicPack, ValidationStatus } from "@/types/topic-packs";

function sourceBadgeClasses(source: TopicPack["source"]): string {
  return source === "bundled"
    ? "bg-primary/10 text-primary"
    : "bg-secondary/40 text-foreground";
}

function validationMeta(
  status: ValidationStatus,
  count: number,
): { label: string; classes: string; icon: typeof CheckCircle2 } {
  if (status === "ok") {
    return { label: "OK", classes: "bg-green-500/10 text-green-600", icon: CheckCircle2 };
  }
  if (status === "warnings") {
    return {
      label: `${count} warning${count === 1 ? "" : "s"}`,
      classes: "bg-amber-500/10 text-amber-600",
      icon: AlertTriangle,
    };
  }
  return {
    label: `${count} error${count === 1 ? "" : "s"}`,
    classes: "bg-destructive/10 text-destructive",
    icon: AlertCircle,
  };
}

export function SettingsTopicPacksSection() {
  const { packs, isLoading, reloading, error, loadPacks, setEnabled, reloadSkills } =
    useTopicPacksStore();
  const [expandedPackIds, setExpandedPackIds] = useState<Set<string>>(new Set());
  const [dismissed, setDismissed] = useState(false);

  // Mount-time load. Store catches its own errors and sets `error`.
  useEffect(() => {
    void loadPacks();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function toggleExpand(packId: string) {
    setExpandedPackIds((prev) => {
      const next = new Set(prev);
      if (next.has(packId)) next.delete(packId);
      else next.add(packId);
      return next;
    });
  }

  const displayedError = dismissed ? null : error;

  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">Topic Packs</h2>
      <p className="text-xs text-muted-foreground">
        Curated packs ship with LearnForge; drop your own under
        {" "}
        <code className="rounded bg-secondary/40 px-1 py-0.5 text-[11px]">
          ~/.learnforge/skills/
        </code>
        .
      </p>

      <div className="glass rounded-xl p-5 space-y-3">
        {/* Header row — title + Reload button */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Package2 size={16} className="text-muted-foreground" />
            <p className="text-sm font-medium text-foreground">Installed packs</p>
            {isLoading && (
              <Loader2 size={14} className="animate-spin text-muted-foreground" />
            )}
          </div>
          <button
            type="button"
            onClick={() => void reloadSkills()}
            disabled={reloading}
            data-testid="reload-skills-button"
            className={cn(
              "flex items-center gap-2 rounded-lg border border-border px-3 py-1.5 text-xs font-medium transition-colors",
              reloading
                ? "cursor-not-allowed opacity-60"
                : "text-foreground hover:bg-secondary",
            )}
          >
            {reloading ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <RefreshCw size={12} />
            )}
            Reload skills
          </button>
        </div>

        {/* Error banner */}
        {displayedError && (
          <div className="flex items-start justify-between gap-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            <span className="flex-1">{displayedError}</span>
            <button
              type="button"
              onClick={() => setDismissed(true)}
              className="text-[11px] font-medium underline underline-offset-2 hover:no-underline"
            >
              Dismiss
            </button>
          </div>
        )}

        {/* Empty state */}
        {!isLoading && packs.length === 0 && (
          <p className="rounded-md border border-dashed border-border/60 px-3 py-4 text-center text-xs text-muted-foreground">
            No packs loaded — check the application logs for validation errors.
          </p>
        )}

        {/* Pack rows */}
        <ul className="space-y-2">
          {packs.map((p) => {
            const id = p.pack.id;
            const meta = validationMeta(p.validationStatus, p.validationMessages.length);
            const isExpanded = expandedPackIds.has(id);
            const MetaIcon = meta.icon;
            return (
              <li
                key={id}
                data-testid={`pack-row-${id}`}
                className={cn(
                  "rounded-lg border border-border/60 bg-secondary/20 px-3 py-2.5",
                  !p.enabled && "opacity-60",
                )}
              >
                <div className="flex items-center justify-between gap-3">
                  {/* Toggle */}
                  <button
                    type="button"
                    role="switch"
                    aria-checked={p.enabled}
                    aria-label={`Toggle pack ${p.pack.title}`}
                    data-testid={`pack-toggle-${id}`}
                    onClick={() => void setEnabled(id, !p.enabled)}
                    className={cn(
                      "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors",
                      p.enabled ? "bg-primary" : "bg-input",
                    )}
                  >
                    <span
                      className={cn(
                        "inline-block h-4 w-4 transform rounded-full bg-white transition-transform",
                        p.enabled ? "translate-x-6" : "translate-x-1",
                      )}
                    />
                  </button>

                  {/* Title + id */}
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-sm font-medium text-foreground">
                      {p.pack.title}
                    </p>
                    <p className="truncate text-[11px] text-muted-foreground">
                      {id}
                    </p>
                  </div>

                  {/* Source badge */}
                  <span
                    data-testid={`pack-source-${id}`}
                    className={cn(
                      "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                      sourceBadgeClasses(p.source),
                    )}
                  >
                    {p.source === "bundled" ? "Bundled" : "Skill"}
                  </span>

                  {/* Validation badge — click to expand when not ok */}
                  <button
                    type="button"
                    onClick={() =>
                      p.validationStatus !== "ok" && toggleExpand(id)
                    }
                    disabled={p.validationStatus === "ok"}
                    data-testid={`pack-validation-${id}`}
                    aria-expanded={isExpanded}
                    className={cn(
                      "flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
                      meta.classes,
                      p.validationStatus !== "ok"
                        ? "cursor-pointer hover:brightness-110"
                        : "cursor-default",
                    )}
                  >
                    <MetaIcon size={11} />
                    {meta.label}
                  </button>
                </div>

                {/* Expandable validation messages */}
                {isExpanded && p.validationMessages.length > 0 && (
                  <ul className="mt-2 space-y-1 border-t border-border/40 pt-2 text-[11px] text-muted-foreground">
                    {p.validationMessages.map((msg, idx) => (
                      <li
                        key={idx}
                        className="font-mono leading-snug break-words"
                      >
                        {msg}
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}
