import { useEffect, useState } from "react";
import * as commands from "@/lib/tauri-commands";
import type { LabRuntimeChoice } from "@/types/learning";
import { Container, Terminal, Sparkles } from "lucide-react";

// ── Phase 03.1 LAB-03 — Labs runtime selector ──
//
// Persists the learner's preferred lab runtime ("docker" | "hostShell" |
// "autoDetect") into `learner_profiles.preferences_json.labs_runtime` per
// 03.1-RESEARCH § Open Question #3 (matches existing per-learner preferences
// pattern; no schema change). Surfaces the resolved Docker availability via
// `lab_runtime_detect` IPC so the learner can see whether the Docker option
// will actually work before selecting it.

const RUNTIME_OPTIONS: ReadonlyArray<{
  value: LabRuntimeChoice;
  label: string;
  description: string;
  icon: typeof Container;
}> = [
  {
    value: "autoDetect",
    label: "Auto-detect",
    description:
      "Use a container when available, fall back to a terminal. Recommended.",
    icon: Sparkles,
  },
  {
    value: "docker",
    label: "Docker",
    description:
      "Always use a containerized sandbox. Requires Desktop or colima.",
    icon: Container,
  },
  {
    value: "hostShell",
    label: "Host shell",
    description: "Always use your local terminal directly. No isolation.",
    icon: Terminal,
  },
];

interface ParsedPreferences {
  labs_runtime?: LabRuntimeChoice;
  [key: string]: unknown;
}

function parsePreferences(json: string | null | undefined): ParsedPreferences {
  if (!json) return {};
  try {
    const parsed = JSON.parse(json);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as ParsedPreferences;
    }
  } catch {
    // Treat malformed JSON as empty preferences — the next save overwrites
    // with a clean object.
  }
  return {};
}

function isLabRuntimeChoice(value: unknown): value is LabRuntimeChoice {
  return value === "docker" || value === "hostShell" || value === "autoDetect";
}

export function SettingsLabsSection() {
  const [runtime, setRuntime] = useState<LabRuntimeChoice>("autoDetect");
  const [dockerAvailable, setDockerAvailable] = useState<boolean | null>(null);
  const [dockerVersion, setDockerVersion] = useState<string | null>(null);
  const [persistError, setPersistError] = useState<string | null>(null);

  // Hydrate persisted preference on mount.
  useEffect(() => {
    let cancelled = false;
    async function hydrate() {
      try {
        const profile = await commands.getOrCreateProfile();
        if (cancelled) return;
        const prefs = parsePreferences(profile.preferencesJson);
        const stored = prefs.labs_runtime;
        if (isLabRuntimeChoice(stored)) {
          setRuntime(stored);
        }
      } catch (err) {
        console.error("SettingsLabsSection: failed to load profile", err);
      }
    }
    hydrate();
    return () => {
      cancelled = true;
    };
  }, []);

  // Re-probe Docker availability whenever the selector changes (on mount too).
  useEffect(() => {
    let cancelled = false;
    async function probe() {
      try {
        const result = await commands.labRuntimeDetect({ setting: runtime });
        if (cancelled) return;
        setDockerAvailable(result.dockerAvailable);
        setDockerVersion(result.dockerVersion ?? null);
      } catch (err) {
        if (cancelled) return;
        console.error("SettingsLabsSection: lab_runtime_detect failed", err);
        setDockerAvailable(false);
        setDockerVersion(null);
      }
    }
    probe();
    return () => {
      cancelled = true;
    };
  }, [runtime]);

  async function persistRuntime(next: LabRuntimeChoice) {
    setPersistError(null);
    try {
      const profile = await commands.getOrCreateProfile();
      const prefs = parsePreferences(profile.preferencesJson);
      const merged = { ...prefs, labs_runtime: next };
      await commands.updateProfile({
        preferencesJson: JSON.stringify(merged),
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setPersistError(`Failed to save preference: ${msg}`);
    }
  }

  function handleSelect(next: LabRuntimeChoice, isDisabled: boolean) {
    if (isDisabled) return;
    if (next === runtime) return;
    setRuntime(next);
    void persistRuntime(next);
  }

  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">Labs</h2>
      <p className="text-xs text-muted-foreground">
        How to run interactive lab terminals
      </p>

      <div className="glass rounded-xl p-5 space-y-5">
        {/* Selector — segmented radio-card list, always visible */}
        <div
          data-testid="labs-runtime-select"
          data-value={runtime}
          role="listbox"
          aria-label="Lab runtime"
          className="space-y-2"
        >
          {RUNTIME_OPTIONS.map((opt) => {
            const isDisabled =
              opt.value === "docker" && dockerAvailable === false;
            const isSelected = runtime === opt.value;
            const Icon = opt.icon;
            return (
              <button
                key={opt.value}
                type="button"
                role="option"
                aria-label={opt.label}
                aria-selected={isSelected}
                aria-disabled={isDisabled}
                disabled={isDisabled}
                onClick={() => handleSelect(opt.value, isDisabled)}
                className={`flex w-full items-start gap-3 rounded-lg border px-3 py-3 text-left transition-colors ${
                  isSelected
                    ? "border-primary/40 bg-primary/10 text-foreground ring-1 ring-primary/30"
                    : "border-border/60 bg-secondary/30 text-muted-foreground hover:border-border hover:text-foreground"
                } ${isDisabled ? "cursor-not-allowed opacity-60" : ""}`}
              >
                <Icon
                  size={16}
                  className="mt-0.5 shrink-0 text-muted-foreground"
                />
                <span className="flex-1">
                  <span className="block text-sm font-medium text-foreground">
                    {opt.label}
                  </span>
                  <span className="mt-0.5 block text-[11px] leading-relaxed text-muted-foreground">
                    {opt.description}
                  </span>
                  {isDisabled && (
                    <span className="mt-1 block text-[11px] font-medium text-amber-500">
                      Not detected on this system
                    </span>
                  )}
                </span>
                {isSelected && (
                  <span
                    aria-hidden="true"
                    className="mt-1 inline-block h-2 w-2 shrink-0 rounded-full bg-primary"
                  />
                )}
              </button>
            );
          })}
        </div>

        {/* Docker availability indicator (testid + data-status; text omits the
            literal label so the section heading + selector option are the
            only `findByText(/docker/i)` matches in this section). */}
        <div
          data-testid="labs-docker-status"
          data-status={
            dockerAvailable === true
              ? "docker-available"
              : "docker-unavailable"
          }
          className="flex items-center gap-2 text-xs"
        >
          <span
            className={`inline-block h-2.5 w-2.5 rounded-full ${
              dockerAvailable === true ? "bg-emerald-500" : "bg-zinc-400"
            }`}
          />
          <span
            className={
              dockerAvailable === true
                ? "font-medium text-emerald-500"
                : "font-medium text-muted-foreground"
            }
          >
            {dockerAvailable === true
              ? `Container engine ready${
                  dockerVersion ? ` (${dockerVersion})` : ""
                }`
              : "Container engine not detected"}
          </span>
        </div>

        {persistError && (
          <p className="text-xs text-destructive">{persistError}</p>
        )}

        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Your selection is saved to your learner profile and applies to all
          new lab sessions.
        </p>
      </div>
    </section>
  );
}
