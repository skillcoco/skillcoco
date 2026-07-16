import type { PathModule, ModuleStatus } from "@/types";

const DEFAULT_ESTIMATED_MINUTES = 30;

/**
 * Parse the backend `modulesJson` string into PathModule[], normalizing the
 * estimatedMinutes field.
 *
 * The backend serializes modulesJson with snake_case `estimated_minutes`
 * (src-tauri/src/commands/ai.rs), while the TS PathModule type expects
 * camelCase `estimatedMinutes`. Reading the camelCase key directly yields
 * `undefined`, which turns sum reduces into `NaN` ("~NaNm estimated
 * remaining") and renders blank per-module estimates. Normalize here so every
 * downstream consumer gets a real number. Returns [] on malformed/empty input.
 */
export function parsePathModules(
  modulesJson: string | null | undefined,
): PathModule[] {
  if (!modulesJson) return [];
  let raw: unknown;
  try {
    raw = JSON.parse(modulesJson);
  } catch {
    return [];
  }
  if (!Array.isArray(raw)) return [];

  return raw.map((entry) => {
    const m = entry as Record<string, unknown> & Partial<PathModule>;
    const camel = m.estimatedMinutes;
    const snake = (m as Record<string, unknown>).estimated_minutes;
    const estimatedMinutes =
      typeof camel === "number" && !Number.isNaN(camel)
        ? camel
        : typeof snake === "number" && !Number.isNaN(snake)
          ? snake
          : DEFAULT_ESTIMATED_MINUTES;
    return { ...(m as PathModule), estimatedMinutes };
  });
}

/** Per-module BKT mastery needed to count a module as "mastered". */
export const MASTERY_GATE = 0.7;
/** Track-wide average mastery the Completion certificate requires. */
export const CERT_AVG_GATE = 0.85;

export interface CertGate {
  modulesTotal: number;
  /** Modules at mastery >= MASTERY_GATE. */
  modulesMastered: number;
  /** Rounded percentage of modules mastered (0..100). */
  masteredPct: number;
  /** Average mastery across ALL modules (missing mastery counts as 0). */
  avgMastery: number;
  /** 100% of modules mastered. */
  meetsModules: boolean;
  /** Average mastery clears the certificate bar. */
  meetsAvg: boolean;
}

/**
 * Compute the Completion-certificate gate status for a track. This is the
 * "more than finishing" signal: the certificate requires 100% of modules
 * mastered AND average mastery >= CERT_AVG_GATE (plus practical labs, gated
 * separately backend-side). Mirrors skillcoco-core::threshold — avg is taken
 * across ALL modules with missing mastery counted as 0.
 */
export function computeCertGate(
  modules: PathModule[],
  masteryOf: (moduleId: string) => number,
): CertGate {
  const modulesTotal = modules.length;
  let modulesMastered = 0;
  let sum = 0;
  for (const m of modules) {
    const lvl = masteryOf(m.id) || 0;
    sum += lvl;
    if (lvl >= MASTERY_GATE) modulesMastered++;
  }
  const avgMastery = modulesTotal === 0 ? 0 : sum / modulesTotal;
  return {
    modulesTotal,
    modulesMastered,
    masteredPct:
      modulesTotal === 0
        ? 0
        : Math.round((modulesMastered / modulesTotal) * 100),
    avgMastery,
    meetsModules: modulesTotal > 0 && modulesMastered === modulesTotal,
    meetsAvg: avgMastery >= CERT_AVG_GATE,
  };
}

/**
 * Pick the next actionable module for a "Continue learning" CTA:
 *   1. the in_progress module, else
 *   2. the first available (unlocked, not completed) module in path order,
 *   3. else null (nothing to resume — all completed or locked).
 */
export function pickNextModule(
  modules: PathModule[],
  statusOf: (moduleId: string) => ModuleStatus,
): PathModule | null {
  return (
    modules.find((m) => statusOf(m.id) === "in_progress") ??
    modules.find((m) => statusOf(m.id) === "available") ??
    null
  );
}
