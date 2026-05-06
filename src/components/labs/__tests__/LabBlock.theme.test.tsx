// Wave 0 failing tests for LabBlock glassmorphism theming (LAB-10 theme
// snapshot). The CSS-variable-driven className assertion lets us check
// theme-aware styling without depending on jsdom canvas rendering.
//
// FAILS today because the LabBlock stub renders a plain placeholder div
// without the glassmorphism container. Plan 03.1-06 makes this green.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const labStoreState = vi.hoisted(() => ({
  openSession: vi.fn().mockResolvedValue({ sessionId: "sess-1", effectiveRuntime: "docker" }),
  closeSession: vi.fn().mockResolvedValue(undefined),
  markStepComplete: vi.fn(),
  getProgress: vi.fn(),
}));

vi.mock("@/stores/useLabStore", () => ({
  useLabStore: vi.fn((selector?: (s: typeof labStoreState) => unknown) =>
    typeof selector === "function" ? selector(labStoreState) : labStoreState,
  ),
  __resetStore: vi.fn(),
}));

const themeState = vi.hoisted(() => ({ theme: "dark" as "dark" | "light" }));

vi.mock("@/hooks/useTheme", () => ({
  useTheme: vi.fn(() => ({
    theme: themeState.theme,
    setTheme: vi.fn(),
    toggleTheme: vi.fn(),
  })),
}));

vi.mock("@/lib/tauri-commands", () => ({
  getLessonCompletions: vi.fn().mockResolvedValue([]),
}));

import { LabBlock } from "@/components/labs/LabBlock";
import type { ModuleBlock } from "@/types/learning";

function makeLabBlock(): ModuleBlock {
  const spec = {
    slug: "pod-inspect",
    title: "Inspect a Pod",
    requiresDocker: true,
    image: "kindest/node:v1.30",
    creates: ["deploy.yaml"],
    steps: [],
  };
  return {
    id: "blk-lab-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "lab",
    status: "ready",
    paramsJson: "{}",
    payloadJson: JSON.stringify({ spec }),
    sourceAnchorsJson: "[]",
    metadataJson: "{}",
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
  };
}

describe("LabBlock theme — Phase 03.1 Wave 0 (LAB-10 glassmorphism)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("lab_block_dark_theme — references --glass-bg and --border-subtle CSS variables", () => {
    // FAILS until 03.1-06 applies glassmorphism tokens to the container.
    themeState.theme = "dark";
    const { container } = render(
      <MemoryRouter>
        <LabBlock block={makeLabBlock()} learnerId="learner-1" trackId="trk-1" />
      </MemoryRouter>,
    );
    const root = container.firstChild as HTMLElement | null;
    expect(root).not.toBeNull();
    // CSS-variable usage shows up in the inline style or in a child wrapper's
    // computed style. Assert the marker class / data attribute is present.
    const wrapper = container.querySelector("[data-glass-surface=\"true\"]");
    expect(wrapper).not.toBeNull();
  });

  it("lab_block_light_theme — switches CSS-variable surface without reset", () => {
    // FAILS until 03.1-06 ties the glass surface to the active theme.
    themeState.theme = "light";
    const { container } = render(
      <MemoryRouter>
        <LabBlock block={makeLabBlock()} learnerId="learner-1" trackId="trk-1" />
      </MemoryRouter>,
    );
    const wrapper = container.querySelector("[data-glass-surface=\"true\"]");
    expect(wrapper).not.toBeNull();
    expect(wrapper?.getAttribute("data-theme")).toBe("light");
  });
});
