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
