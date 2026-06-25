// Phase 08.3 Wave 2 — TopicPicker tests (renamed from PackPicker).
//
// Covers D-04..D-08 (topic-first refactor: free-text primary + chip
// cloud + collapsible templates section + no emojis). Replaces Phase 5
// PackPicker tests for the same component. Carries forward
// PackPickerCertPreview integration (D-10).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TopicPicker, CHIP_TOPICS } from "@/components/onboarding/TopicPicker";
import type { TopicPack } from "@/types/topic-packs";

vi.mock("@/lib/tauri-commands", () => ({
  listTopicPacks: vi.fn(),
}));

import { listTopicPacks } from "@/lib/tauri-commands";

const listTopicPacksMock = vi.mocked(listTopicPacks);

function makePack(id: string, source: "bundled" | "skill"): TopicPack {
  return {
    pack: {
      id,
      title: `${id} title`,
      description: `description for ${id}`,
      domain_module: "devops",
      estimated_hours: 8,
      pack_version: "1.0",
      requires_docker: false,
      modules: [],
      edges: [],
    },
    source,
    enabled: true,
    validationStatus: "ok",
    validationMessages: [],
    lastLoadedAt: "2026-06-15T00:00:00Z",
  };
}

describe("TopicPicker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── Primary surface: free-text input (D-04) ─────────────────────────
  it("renders free-text input as the primary surface at top of step", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    // Heading
    expect(screen.getByText(/what do you want to learn\?/i)).toBeInTheDocument();
    // Free-text input is present and focused on mount.
    const input = screen.getByTestId("topic-freetext-input");
    expect(input).toBeInTheDocument();
    expect(input).toHaveFocus();
    // Reassurance line
    expect(
      screen.getByText(/ai will build a personalized path for any topic/i),
    ).toBeInTheDocument();
  });

  it("has 'e.g. Spanish, Watercolor, Python...' placeholder per D-04", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    const input = screen.getByTestId("topic-freetext-input") as HTMLInputElement;
    expect(input.placeholder).toMatch(/spanish/i);
    expect(input.placeholder).toMatch(/watercolor/i);
    expect(input.placeholder).toMatch(/python/i);
  });

  // ── Chip cloud (D-05) ───────────────────────────────────────────────
  it("renders 12 diverse chips below the free-text input", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    // Exact count per D-05.
    expect(CHIP_TOPICS).toHaveLength(12);

    for (const topic of CHIP_TOPICS) {
      expect(screen.getByTestId(`chip-${topic.slug}`)).toBeInTheDocument();
    }
  });

  it("chip cloud includes diverse mix (tech + creative + practical + academic + lifestyle + language)", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    // Tech
    expect(screen.getByText("Python")).toBeInTheDocument();
    expect(screen.getByText("Kubernetes")).toBeInTheDocument();
    expect(screen.getByText("JavaScript")).toBeInTheDocument();
    // Creative
    expect(screen.getByText("Watercolor")).toBeInTheDocument();
    expect(screen.getByText("Music Theory")).toBeInTheDocument();
    expect(screen.getByText("Photography")).toBeInTheDocument();
    // Practical
    expect(screen.getByText("Cooking")).toBeInTheDocument();
    expect(screen.getByText("Public Speaking")).toBeInTheDocument();
    // Academic
    expect(screen.getByText("Algebra")).toBeInTheDocument();
    expect(screen.getByText("History")).toBeInTheDocument();
    // Lifestyle
    expect(screen.getByText("Wine Tasting")).toBeInTheDocument();
    // Language
    expect(screen.getByText("Spanish")).toBeInTheDocument();
  });

  it("clicking a chip prefills the free-text input", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await user.click(screen.getByTestId("chip-spanish"));

    const input = screen.getByTestId("topic-freetext-input") as HTMLInputElement;
    expect(input.value).toBe("Spanish");
  });

  it("chips contain only plain text — NO emojis (D-08)", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    // Regex matches unicode "Extended_Pictographic" emoji classes. Any
    // emoji on a chip would fail this.
    const emojiPattern = /\p{Extended_Pictographic}/u;
    for (const topic of CHIP_TOPICS) {
      const chip = screen.getByTestId(`chip-${topic.slug}`);
      expect(chip.textContent ?? "").not.toMatch(emojiPattern);
    }
  });

  // ── Continue button gating (D-04 + plan rule) ───────────────────────
  it("Continue button is disabled when free-text input is empty", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    const submit = screen.getByTestId("topic-freetext-submit");
    expect(submit).toBeDisabled();
  });

  it("Continue button enables once free-text has content", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await user.type(screen.getByTestId("topic-freetext-input"), "Spanish");

    expect(screen.getByTestId("topic-freetext-submit")).not.toBeDisabled();
  });

  it("submitting free-text calls onCustomTopic with the typed topic", async () => {
    const user = userEvent.setup();
    const onCustomTopic = vi.fn();
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={onCustomTopic} />);

    await user.type(screen.getByTestId("topic-freetext-input"), "Distributed Systems");
    await user.click(screen.getByTestId("topic-freetext-submit"));

    // Phase 08.3: domain is no longer surfaced to learners; PageBuilder
    // detects technical topics. We pass a generic domain placeholder.
    expect(onCustomTopic).toHaveBeenCalledWith("Distributed Systems", expect.any(String));
  });

  // ── Templates collapsible (D-06) ───────────────────────────────────
  it("templates section is collapsed by default", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });

    // Toggle is present, but pack cards are NOT in the DOM yet.
    expect(
      screen.queryByTestId("pack-card-kubernetes-fundamentals"),
    ).not.toBeInTheDocument();
  });

  it("templates header reads 'Or use a curated template'", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });
    expect(screen.getByText(/or use a curated template/i)).toBeInTheDocument();
  });

  it("expanding templates reveals bundled packs + skills", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("rust-from-zero", "bundled"),
      makePack("my-custom-skill", "skill"),
    ]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("templates-toggle"));

    // Bundled packs visible
    expect(screen.getByTestId("pack-card-kubernetes-fundamentals")).toBeInTheDocument();
    expect(screen.getByTestId("pack-card-rust-from-zero")).toBeInTheDocument();
    // My Skills section visible
    expect(screen.getByText(/my skills/i)).toBeInTheDocument();
    expect(screen.getByTestId("pack-card-my-custom-skill")).toBeInTheDocument();
  });

  it("clicking a template pack calls onPick with packId/topic/domainModule", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    render(<TopicPicker onPick={onPick} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("templates-toggle"));
    await user.click(screen.getByTestId("pack-card-kubernetes-fundamentals"));

    expect(onPick).toHaveBeenCalledWith(
      "kubernetes-fundamentals",
      "kubernetes-fundamentals title",
      "devops",
    );
  });

  it("expanding templates loads packs on demand (or earlier — listTopicPacks called)", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(listTopicPacksMock).toHaveBeenCalled();
    });
  });

  // ── PackPickerCertPreview integration (D-10 — carried forward) ──────
  it("renders PackPickerCertPreview on each pack tile in expanded templates", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("rust-from-zero", "bundled"),
    ]);
    render(<TopicPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });
    await user.click(screen.getByTestId("templates-toggle"));

    const previews = screen.getAllByTestId("pack-picker-cert-preview");
    expect(previews).toHaveLength(2);
    expect(
      screen.getAllByText(/1 completion certificate available/i).length,
    ).toBeGreaterThanOrEqual(2);
  });
});
