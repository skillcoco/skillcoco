// Phase 08.3 (Positioning + Onboarding refactor) — TopicPicker component.
//
// Replaces the Phase 5 pack-first PackPicker with a topic-first surface:
//   1. Free-text input (primary, top, focus-on-mount) — D-04
//   2. Chip cloud of 12 diverse topics — D-05
//   3. Templates section (collapsible, demoted) — D-06 + D-07
//
// Preserves the legacy onPick(packId, topic, domainModule) /
// onCustomTopic(topic, domain) callback contract so Onboarding.tsx
// continues to thread packId into generateLearningPath.
//
// No emojis (D-08). Chips use plain text + outline borders.

import { useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  Package2,
  Sparkles,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { listTopicPacks } from "@/lib/tauri-commands";
import type { TopicPack } from "@/types/topic-packs";
import { PackPickerCertPreview } from "@/components/achievements/PackPickerCertPreview";

// D-05 — exact list of 12 chips, balanced across subject categories so the
// chip cloud signals "this app does anything" rather than biasing to
// programming. Exported so the test suite can iterate the canonical set
// instead of duplicating the strings.
export interface ChipTopic {
  slug: string;
  label: string;
}

export const CHIP_TOPICS: readonly ChipTopic[] = [
  // Language
  { slug: "spanish", label: "Spanish" },
  // Creative
  { slug: "watercolor", label: "Watercolor" },
  // Tech
  { slug: "python", label: "Python" },
  // Creative
  { slug: "music-theory", label: "Music Theory" },
  // Tech
  { slug: "kubernetes", label: "Kubernetes" },
  // Academic
  { slug: "algebra", label: "Algebra" },
  // Creative
  { slug: "photography", label: "Photography" },
  // Practical
  { slug: "cooking", label: "Cooking" },
  // Practical
  { slug: "public-speaking", label: "Public Speaking" },
  // Tech
  { slug: "javascript", label: "JavaScript" },
  // Academic
  { slug: "history", label: "History" },
  // Lifestyle
  { slug: "wine-tasting", label: "Wine Tasting" },
] as const;

// Phase 08.3 — domain hint passed through for backward compatibility
// with onCustomTopic(topic, domain). Onboarding step 2 used to surface a
// 5-domain picker; we removed it in favor of trusting the AI / Page
// Planner to detect technical topics. "general" preserves the camelCase
// + non-empty contract without re-introducing the picker.
const DEFAULT_DOMAIN = "general";

export interface TopicPickerProps {
  onPick: (packId: string, packTopic: string, domainModule: string) => void;
  onCustomTopic: (topic: string, domain: string) => void;
}

export function TopicPicker({ onPick, onCustomTopic }: TopicPickerProps) {
  const [packs, setPacks] = useState<TopicPack[]>([]);
  const [isLoadingPacks, setIsLoadingPacks] = useState(true);
  const [packLoadError, setPackLoadError] = useState<string | null>(null);
  const [topic, setTopic] = useState("");
  const [templatesOpen, setTemplatesOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Focus the free-text input on mount — it is the primary surface
  // (D-04). autoFocus on the input would also work but a ref keeps
  // focus management explicit and survives any future refactor that
  // wraps the input in a conditional.
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Fetch packs eagerly on mount so the templates section is responsive
  // when the learner expands it. Net wire cost is identical to Phase 5;
  // only the UI surface that consumes the result moved.
  useEffect(() => {
    let cancelled = false;
    setIsLoadingPacks(true);
    setPackLoadError(null);
    listTopicPacks()
      .then((p) => {
        if (!cancelled) {
          setPacks(p);
          setIsLoadingPacks(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setPackLoadError(String(err));
          setPacks([]);
          setIsLoadingPacks(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const bundled = packs.filter((p) => p.source === "bundled");
  const skills = packs.filter((p) => p.source === "skill");

  function handleChipClick(label: string) {
    setTopic(label);
    inputRef.current?.focus();
  }

  function handleSubmit() {
    const trimmed = topic.trim();
    if (!trimmed) return;
    onCustomTopic(trimmed, DEFAULT_DOMAIN);
  }

  return (
    <div className="glass rounded-xl p-6 space-y-6">
      {/* ── Primary surface: free-text input (D-04) ─────────────────── */}
      <section className="space-y-3">
        <h2 className="text-lg font-semibold text-foreground">
          What do you want to learn?
        </h2>
        <input
          ref={inputRef}
          type="text"
          data-testid="topic-freetext-input"
          value={topic}
          onChange={(e) => setTopic(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              handleSubmit();
            }
          }}
          placeholder="e.g. Spanish, Watercolor, Python..."
          className="w-full rounded-lg border border-input bg-background px-4 py-3 text-base focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
        />
        <p className="text-xs text-muted-foreground">
          AI will build a personalized path for any topic.
        </p>

        {/* ── Chip cloud (D-05) ──────────────────────────────────────── */}
        <div className="flex flex-wrap gap-2 pt-1">
          {CHIP_TOPICS.map((t) => (
            <button
              key={t.slug}
              type="button"
              data-testid={`chip-${t.slug}`}
              onClick={() => handleChipClick(t.label)}
              className="rounded-full border border-border bg-background/40 px-3 py-1 text-xs font-medium text-foreground transition-colors hover:border-primary hover:bg-primary/10 hover:text-primary"
            >
              {t.label}
            </button>
          ))}
        </div>

        <button
          type="button"
          data-testid="topic-freetext-submit"
          onClick={handleSubmit}
          disabled={!topic.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          Continue
          <ArrowRight size={16} />
        </button>
      </section>

      {/* ── Templates section (D-06 + D-07) ─────────────────────────── */}
      <section className="space-y-3 border-t border-border pt-4">
        <button
          type="button"
          onClick={() => setTemplatesOpen((o) => !o)}
          data-testid="templates-toggle"
          className="flex w-full items-center gap-2 text-sm font-medium text-muted-foreground hover:text-foreground"
        >
          {templatesOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          Or use a curated template
        </button>

        {templatesOpen && (
          <div className="space-y-4">
            {packLoadError && (
              <p className="text-xs text-destructive">
                Could not load templates: {packLoadError}
              </p>
            )}

            {/* Bundled templates */}
            <div className="space-y-2">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Templates
              </h3>
              {isLoadingPacks ? (
                <p className="text-xs text-muted-foreground">Loading templates…</p>
              ) : bundled.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No bundled templates loaded. Check application logs for
                  validation errors.
                </p>
              ) : (
                <div className="grid gap-2 sm:grid-cols-2">
                  {bundled.map((p) => (
                    <PackCard
                      key={p.pack.id}
                      pack={p}
                      onPick={onPick}
                      showSourceBadge={false}
                    />
                  ))}
                </div>
              )}
            </div>

            {/* My Skills (D-07) */}
            <div className="space-y-2">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                My Skills
              </h3>
              {skills.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  Drop a pack.json under{" "}
                  <code className="rounded bg-secondary/40 px-1 py-0.5 text-[11px]">
                    ~/.skillcoco/skills/
                  </code>{" "}
                  to add your own template.
                </p>
              ) : (
                <div className="grid gap-2 sm:grid-cols-2">
                  {skills.map((p) => (
                    <PackCard
                      key={p.pack.id}
                      pack={p}
                      onPick={onPick}
                      showSourceBadge
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}

function PackCard({
  pack,
  onPick,
  showSourceBadge,
}: {
  pack: TopicPack;
  onPick: (packId: string, packTopic: string, domainModule: string) => void;
  showSourceBadge: boolean;
}) {
  const moduleCount = pack.pack.modules.length;
  const handlePick = () =>
    onPick(pack.pack.id, pack.pack.title, pack.pack.domain_module);
  // Plan 06-05 (Wave 4) — outer is <div role="button"> to host the
  // PackPickerCertPreview expand toggle as a nested button (HTML
  // disallows button-inside-button; React also warns at runtime).
  return (
    <div
      data-testid={`pack-card-${pack.pack.id}`}
      role="button"
      tabIndex={0}
      onClick={handlePick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handlePick();
        }
      }}
      className={cn(
        "flex cursor-pointer flex-col items-start gap-1 rounded-lg border border-border bg-background p-3 text-left transition-colors hover:border-primary/50 hover:bg-accent/30",
      )}
    >
      <div className="flex w-full items-start justify-between gap-2">
        <span className="text-sm font-medium text-foreground">
          {pack.pack.title}
        </span>
        {showSourceBadge && (
          <span className="rounded-full bg-secondary/40 px-2 py-0.5 text-[10px] font-medium text-foreground">
            Skill
          </span>
        )}
      </div>
      <span className="text-xs text-muted-foreground line-clamp-2">
        {pack.pack.description}
      </span>
      <div className="mt-1 flex items-center gap-3 text-[11px] text-muted-foreground">
        <span className="flex items-center gap-1">
          <Package2 size={11} />
          {moduleCount} module{moduleCount === 1 ? "" : "s"}
        </span>
        {pack.pack.estimated_hours != null && (
          <span className="flex items-center gap-1">
            <Sparkles size={11} />~{pack.pack.estimated_hours}h
          </span>
        )}
      </div>
      <div
        className="mt-2 w-full"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <PackPickerCertPreview moduleCount={moduleCount} />
      </div>
    </div>
  );
}
