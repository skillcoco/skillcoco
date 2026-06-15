// Phase 5 Plan 05 (Wave 4) — PackPicker tests.
//
// Covers D-08 (two grouped sections + collapsible custom-topic fallback),
// R1 (source attribution implicit in section grouping; explicit on TrackView
// in Task 3), R5 (free-text fallback preserved). 8 GREEN tests.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PackPicker } from "@/components/onboarding/PackPicker";
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

describe("PackPicker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads enabled packs on mount", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(listTopicPacksMock).toHaveBeenCalledTimes(1);
    });
  });

  it("renders Topic Packs section with bundled packs", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("rust-from-zero", "bundled"),
    ]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/topic packs/i)).toBeInTheDocument();
      expect(screen.getByTestId("pack-card-kubernetes-fundamentals")).toBeInTheDocument();
      expect(screen.getByTestId("pack-card-rust-from-zero")).toBeInTheDocument();
    });
  });

  it("renders My Skills section with skill packs", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("my-custom-skill", "skill"),
    ]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/my skills/i)).toBeInTheDocument();
      expect(screen.getByTestId("pack-card-my-custom-skill")).toBeInTheDocument();
    });
  });

  it("My Skills empty state shows drop instructions", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/~\/\.learnforge\/skills\//)).toBeInTheDocument();
    });
  });

  it("clicking a pack card calls onPick with packId/topic/domainModule", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    render(<PackPicker onPick={onPick} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("pack-card-kubernetes-fundamentals")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("pack-card-kubernetes-fundamentals"));

    expect(onPick).toHaveBeenCalledWith(
      "kubernetes-fundamentals",
      "kubernetes-fundamentals title",
      "devops",
    );
  });

  it("collapsible custom-topic fallback is closed by default", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("custom-topic-toggle")).toBeInTheDocument();
    });

    // The form input should NOT be in the document while collapsed.
    expect(screen.queryByPlaceholderText(/kubernetes/i)).not.toBeInTheDocument();
  });

  it("expanding collapsible reveals text input + domain buttons", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId("custom-topic-toggle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("custom-topic-toggle"));

    expect(screen.getByPlaceholderText(/kubernetes/i)).toBeInTheDocument();
    expect(screen.getByText(/programming language/i)).toBeInTheDocument();
    expect(screen.getByText(/devops & infrastructure/i)).toBeInTheDocument();
  });

  it("submitting custom-topic form calls onCustomTopic", async () => {
    const user = userEvent.setup();
    const onCustomTopic = vi.fn();
    listTopicPacksMock.mockResolvedValue([]);
    render(<PackPicker onPick={vi.fn()} onCustomTopic={onCustomTopic} />);

    await waitFor(() => {
      expect(screen.getByTestId("custom-topic-toggle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Distributed systems");
    await user.click(screen.getByText(/concepts & theory/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    expect(onCustomTopic).toHaveBeenCalledWith("Distributed systems", "concepts");
  });
});
