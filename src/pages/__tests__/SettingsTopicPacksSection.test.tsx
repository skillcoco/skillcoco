// Phase 5 Plan 04 (Wave 3) — SettingsTopicPacksSection GREEN tests.
//
// Three RED scaffolds from Wave 0 turn GREEN here + two new tests for
// (a) expandable validation messages, (b) reduced-opacity on disabled packs.
//
// We mock the Zustand store directly so each test controls the state shape
// and can assert on which action was invoked. The hook is a function that
// returns the state object — vi.mocked + mockReturnValue handles that.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/stores/useTopicPacksStore", () => ({
  useTopicPacksStore: vi.fn(),
}));

import { useTopicPacksStore } from "@/stores/useTopicPacksStore";
import { SettingsTopicPacksSection } from "@/pages/SettingsTopicPacksSection";
import type { TopicPack, PackSource, ValidationStatus } from "@/types/topic-packs";

function makePack(
  id: string,
  opts: {
    title?: string;
    source?: PackSource;
    enabled?: boolean;
    validationStatus?: ValidationStatus;
    validationMessages?: string[];
  } = {},
): TopicPack {
  return {
    pack: {
      id,
      title: opts.title ?? `Pack ${id}`,
      description: `Description for ${id}`,
      domain_module: "devops",
      pack_version: "1.0",
      requires_docker: false,
      modules: [],
      edges: [],
    },
    source: opts.source ?? "bundled",
    enabled: opts.enabled ?? true,
    validationStatus: opts.validationStatus ?? "ok",
    validationMessages: opts.validationMessages ?? [],
    lastLoadedAt: "2026-06-15T12:00:00Z",
  };
}

function mountStore(overrides: {
  packs?: TopicPack[];
  isLoading?: boolean;
  reloading?: boolean;
  error?: string | null;
  loadPacks?: () => Promise<void>;
  setEnabled?: (packId: string, enabled: boolean) => Promise<void>;
  reloadSkills?: () => Promise<void>;
} = {}) {
  const state = {
    packs: overrides.packs ?? [],
    isLoading: overrides.isLoading ?? false,
    reloading: overrides.reloading ?? false,
    error: overrides.error ?? null,
    loadPacks: overrides.loadPacks ?? vi.fn().mockResolvedValue(undefined),
    setEnabled: overrides.setEnabled ?? vi.fn().mockResolvedValue(undefined),
    reloadSkills:
      overrides.reloadSkills ?? vi.fn().mockResolvedValue(undefined),
  };
  vi.mocked(useTopicPacksStore).mockReturnValue(state);
  return state;
}

describe("SettingsTopicPacksSection (Wave 3 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders pack list with id, title, source", () => {
    mountStore({
      packs: [
        makePack("k8s", { title: "Kubernetes Fundamentals", source: "bundled" }),
        makePack("my-thing", { title: "My Custom Track", source: "skill" }),
      ],
    });

    render(<SettingsTopicPacksSection />);

    // Titles
    expect(screen.getByText("Kubernetes Fundamentals")).toBeInTheDocument();
    expect(screen.getByText("My Custom Track")).toBeInTheDocument();
    // IDs (kebab-case, rendered subtly)
    expect(screen.getByText("k8s")).toBeInTheDocument();
    expect(screen.getByText("my-thing")).toBeInTheDocument();
    // Source badges per-row — testids defined in the plan
    expect(screen.getByTestId("pack-source-k8s")).toHaveTextContent(/bundled/i);
    expect(screen.getByTestId("pack-source-my-thing")).toHaveTextContent(
      /skill/i,
    );
  });

  it("toggles a pack via setTopicPackEnabled IPC", async () => {
    const setEnabled = vi.fn().mockResolvedValue(undefined);
    mountStore({
      packs: [makePack("k8s", { enabled: true })],
      setEnabled,
    });

    render(<SettingsTopicPacksSection />);

    const toggle = screen.getByTestId("pack-toggle-k8s");
    await userEvent.click(toggle);

    expect(setEnabled).toHaveBeenCalledWith("k8s", false);
  });

  it("triggers reloadSkills IPC on Reload button click", async () => {
    const reloadSkills = vi.fn().mockResolvedValue(undefined);
    mountStore({ packs: [makePack("k8s")], reloadSkills });

    render(<SettingsTopicPacksSection />);

    const button = screen.getByRole("button", { name: /reload skills/i });
    await userEvent.click(button);

    expect(reloadSkills).toHaveBeenCalledTimes(1);
  });

  it("expandable validation messages render when status != ok", async () => {
    mountStore({
      packs: [
        makePack("rust", {
          validationStatus: "warnings",
          validationMessages: ["/modules/0/difficulty: out of range"],
        }),
      ],
    });

    render(<SettingsTopicPacksSection />);

    // Message hidden initially
    expect(
      screen.queryByText("/modules/0/difficulty: out of range"),
    ).not.toBeInTheDocument();

    // Click the validation status badge to expand
    const statusBadge = screen.getByTestId("pack-validation-rust");
    await userEvent.click(statusBadge);

    expect(
      screen.getByText("/modules/0/difficulty: out of range"),
    ).toBeInTheDocument();
  });

  it("disabled pack appears with reduced opacity", () => {
    mountStore({ packs: [makePack("k8s", { enabled: false })] });

    render(<SettingsTopicPacksSection />);

    const row = screen.getByTestId("pack-row-k8s");
    expect(row.className).toMatch(/opacity-/);
  });
});
