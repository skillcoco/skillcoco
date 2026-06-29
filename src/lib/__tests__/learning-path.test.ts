import { describe, it, expect } from "vitest";
import { parsePathModules, pickNextModule } from "@/lib/learning-path";
import type { ModuleStatus } from "@/types";

const baseModuleJson = (over: Record<string, unknown> = {}) => ({
  id: "m1",
  title: "Intro",
  description: "d",
  type: "lesson",
  difficulty: 1,
  objectives: [],
  prerequisites: [],
  ...over,
});

describe("parsePathModules", () => {
  it("maps snake_case estimated_minutes → estimatedMinutes (the NaN bug)", () => {
    const json = JSON.stringify([baseModuleJson({ estimated_minutes: 45 })]);
    const mods = parsePathModules(json);
    expect(mods[0].estimatedMinutes).toBe(45);
  });

  it("passes through camelCase estimatedMinutes", () => {
    const json = JSON.stringify([baseModuleJson({ estimatedMinutes: 20 })]);
    expect(parsePathModules(json)[0].estimatedMinutes).toBe(20);
  });

  it("defaults to 30 when neither casing is present (never undefined → no NaN)", () => {
    const json = JSON.stringify([baseModuleJson()]);
    const v = parsePathModules(json)[0].estimatedMinutes;
    expect(v).toBe(30);
    expect(Number.isNaN(v)).toBe(false);
  });

  it("preserves other fields", () => {
    const json = JSON.stringify([baseModuleJson({ id: "abc", title: "T" })]);
    const m = parsePathModules(json)[0];
    expect(m.id).toBe("abc");
    expect(m.title).toBe("T");
  });

  it("returns [] for invalid JSON / null / empty", () => {
    expect(parsePathModules("not json")).toEqual([]);
    expect(parsePathModules(null)).toEqual([]);
    expect(parsePathModules(undefined)).toEqual([]);
    expect(parsePathModules("")).toEqual([]);
  });

  it("a summed reduce over results never yields NaN", () => {
    const json = JSON.stringify([
      baseModuleJson({ id: "a", estimated_minutes: 10 }),
      baseModuleJson({ id: "b" }), // missing → 30
      baseModuleJson({ id: "c", estimatedMinutes: 5 }),
    ]);
    const total = parsePathModules(json).reduce((acc, m) => acc + m.estimatedMinutes, 0);
    expect(total).toBe(45);
    expect(Number.isNaN(total)).toBe(false);
  });
});

describe("pickNextModule", () => {
  const mods = parsePathModules(
    JSON.stringify([
      baseModuleJson({ id: "a" }),
      baseModuleJson({ id: "b" }),
      baseModuleJson({ id: "c" }),
    ]),
  );
  const statusMap = (m: Record<string, ModuleStatus>) => (id: string) =>
    m[id] ?? "locked";

  it("prefers the in_progress module over available", () => {
    const next = pickNextModule(mods, statusMap({ a: "completed", b: "in_progress", c: "available" }));
    expect(next?.id).toBe("b");
  });

  it("falls back to the first available when none in_progress", () => {
    const next = pickNextModule(mods, statusMap({ a: "completed", b: "available", c: "available" }));
    expect(next?.id).toBe("b");
  });

  it("returns null when all completed or locked", () => {
    expect(pickNextModule(mods, statusMap({ a: "completed", b: "completed", c: "locked" }))).toBeNull();
  });

  it("returns null for empty module list", () => {
    expect(pickNextModule([], () => "locked")).toBeNull();
  });
});
