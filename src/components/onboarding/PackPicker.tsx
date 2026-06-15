// Phase 5 Plan 05 (Wave 4) — PackPicker component.
//
// Implements D-08: two grouped sections ("Topic Packs" / "My Skills") +
// collapsible "Or describe your own" free-text fallback. Free-text path
// preserves the legacy domain-selector behavior previously embedded in
// Onboarding.tsx (R5 — `domainModules` constant moved here as the single
// canonical source of truth).
//
// Data source: `listTopicPacks()` (enabled-only) via a mount-time effect —
// kept LOCAL to this component to avoid coupling Onboarding to Settings'
// `useTopicPacksStore` (which uses `listTopicPacksAdmin`, the wrong shape
// for the picker).

import { useEffect, useState } from "react";
import { ArrowRight, ChevronDown, ChevronRight, Package2, Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";
import { listTopicPacks } from "@/lib/tauri-commands";
import type { TopicPack } from "@/types/topic-packs";

// Canonical domain list — was previously in Onboarding.tsx (R5). The free-
// text fallback inside the picker is now its sole authoritative source.
const DOMAIN_MODULES = [
  { id: "programming", label: "Programming Language", examples: "Rust, Go, Python, TypeScript" },
  { id: "devops", label: "DevOps & Infrastructure", examples: "Kubernetes, Docker, Terraform, CI/CD" },
  { id: "cloud", label: "Cloud Platforms", examples: "AWS, GCP, Azure" },
  { id: "concepts", label: "Concepts & Theory", examples: "System Design, Algorithms, Networking" },
  { id: "data", label: "Data & AI/ML", examples: "ML Engineering, Data Pipelines, LLMs" },
];

export interface PackPickerProps {
  onPick: (packId: string, packTopic: string, domainModule: string) => void;
  onCustomTopic: (topic: string, domain: string) => void;
}

export function PackPicker({ onPick, onCustomTopic }: PackPickerProps) {
  const [packs, setPacks] = useState<TopicPack[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [customTopic, setCustomTopic] = useState("");
  const [customDomain, setCustomDomain] = useState("");

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    setLoadError(null);
    listTopicPacks()
      .then((p) => {
        if (!cancelled) {
          setPacks(p);
          setIsLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setLoadError(String(err));
          setPacks([]);
          setIsLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const bundled = packs.filter((p) => p.source === "bundled");
  const skills = packs.filter((p) => p.source === "skill");

  function handleCustomSubmit() {
    if (!customTopic || !customDomain) return;
    onCustomTopic(customTopic, customDomain);
  }

  return (
    <div className="glass rounded-xl p-6 space-y-6">
      {loadError && (
        <p className="text-xs text-destructive">
          Could not load topic packs: {loadError}
        </p>
      )}

      {/* ── Topic Packs section ────────────────────────────────────────── */}
      <section className="space-y-3">
        <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
          Topic Packs
        </h3>
        {isLoading ? (
          <p className="text-xs text-muted-foreground">Loading packs…</p>
        ) : bundled.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            No bundled packs loaded. Check application logs for validation errors.
          </p>
        ) : (
          <div className="grid gap-2 sm:grid-cols-2">
            {bundled.map((p) => (
              <PackCard key={p.pack.id} pack={p} onPick={onPick} showSourceBadge={false} />
            ))}
          </div>
        )}
      </section>

      {/* ── My Skills section ──────────────────────────────────────────── */}
      <section className="space-y-3">
        <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
          My Skills
        </h3>
        {skills.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            Drop a pack.json under{" "}
            <code className="rounded bg-secondary/40 px-1 py-0.5 text-[11px]">
              ~/.learnforge/skills/
            </code>{" "}
            to add your own track.
          </p>
        ) : (
          <div className="grid gap-2 sm:grid-cols-2">
            {skills.map((p) => (
              <PackCard key={p.pack.id} pack={p} onPick={onPick} showSourceBadge />
            ))}
          </div>
        )}
      </section>

      {/* ── Collapsible custom-topic fallback ──────────────────────────── */}
      <section className="space-y-3 border-t border-border pt-4">
        <button
          type="button"
          onClick={() => setExpanded((e) => !e)}
          data-testid="custom-topic-toggle"
          className="flex w-full items-center gap-2 text-sm font-medium text-foreground hover:text-primary"
        >
          {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          Or describe your own
        </button>

        {expanded && (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                What do you want to learn?
              </label>
              <input
                type="text"
                value={customTopic}
                onChange={(e) => setCustomTopic(e.target.value)}
                placeholder="e.g., Kubernetes, Rust programming, System Design…"
                className="w-full rounded-lg border border-input bg-background px-4 py-3 text-sm focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                Domain
              </label>
              <div className="grid gap-2">
                {DOMAIN_MODULES.map((dm) => (
                  <button
                    key={dm.id}
                    type="button"
                    onClick={() => setCustomDomain(dm.id)}
                    className={cn(
                      "flex flex-col items-start rounded-lg border p-3 text-left transition-colors",
                      customDomain === dm.id
                        ? "border-primary bg-primary/5"
                        : "border-border hover:border-primary/50",
                    )}
                  >
                    <span className="font-medium text-foreground">{dm.label}</span>
                    <span className="text-xs text-muted-foreground">{dm.examples}</span>
                  </button>
                ))}
              </div>
            </div>
            <button
              type="button"
              onClick={handleCustomSubmit}
              disabled={!customTopic || !customDomain}
              data-testid="custom-topic-submit"
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              Continue
              <ArrowRight size={16} />
            </button>
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
  return (
    <button
      type="button"
      onClick={() => onPick(pack.pack.id, pack.pack.title, pack.pack.domain_module)}
      data-testid={`pack-card-${pack.pack.id}`}
      className="flex flex-col items-start gap-1 rounded-lg border border-border bg-background p-3 text-left transition-colors hover:border-primary/50 hover:bg-accent/30"
    >
      <div className="flex w-full items-start justify-between gap-2">
        <span className="font-medium text-foreground">{pack.pack.title}</span>
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
            <Sparkles size={11} />
            ~{pack.pack.estimated_hours}h
          </span>
        )}
      </div>
    </button>
  );
}
