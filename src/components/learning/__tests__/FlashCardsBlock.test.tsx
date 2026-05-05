import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Vitest hoisting rule: inline literals only inside vi.mock factory.
vi.mock("@/lib/tauri-commands", () => ({
  rateFlashCard: vi.fn().mockResolvedValue({ masteryLevel: 0.75 }),
}));

import { FlashCardsBlock } from "@/components/learning/FlashCardsBlock";
import { rateFlashCard } from "@/lib/tauri-commands";
import type { ModuleBlock } from "@/types/learning";

function makeFlashBlock(cards: { id: string; front: string; back: string }[]): ModuleBlock {
  return {
    id: "blk-fc-1",
    moduleId: "mod-1",
    ordering: 5,
    blockType: "flash_cards",
    status: "ready",
    paramsJson: `{"card_count":${cards.length}}`,
    payloadJson: JSON.stringify({ cards }),
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
  };
}

const singleCard = [
  { id: "fc-1", front: "What is a Pod?", back: "The smallest deployable unit in Kubernetes." },
];

const threeCards = [
  { id: "fc-1", front: "What is a Pod?", back: "The smallest deployable unit in Kubernetes." },
  { id: "fc-2", front: "What is a Deployment?", back: "Manages a set of replica Pods." },
  { id: "fc-3", front: "What is a Service?", back: "Exposes a set of Pods as a network service." },
];

describe("FlashCardsBlock Phase 3", () => {
  it("flash_card_flip — click card front to reveal back (CSS flipped class applied)", async () => {
    const user = userEvent.setup();
    render(<FlashCardsBlock block={makeFlashBlock(singleCard)} moduleId="mod-1" />);

    const cardFront = screen.getByText("What is a Pod?");
    expect(cardFront).toBeInTheDocument();

    const flipCard = cardFront.closest(".flip-card");
    expect(flipCard).toBeTruthy();
    expect(flipCard).not.toHaveClass("flipped");

    await user.click(flipCard!);

    expect(flipCard).toHaveClass("flipped");
  });

  it("flash_card_renders_multiple_cards — 3 cards renders 3 flip-card elements", () => {
    render(<FlashCardsBlock block={makeFlashBlock(threeCards)} moduleId="mod-1" />);

    const cards = screen.getAllByTestId("flash-card");
    expect(cards).toHaveLength(3);
    expect(screen.getByText("What is a Pod?")).toBeInTheDocument();
    expect(screen.getByText("What is a Deployment?")).toBeInTheDocument();
    expect(screen.getByText("What is a Service?")).toBeInTheDocument();
  });

  it("flash_card_quality_button_calls_rate — click Good button calls rateFlashCard with quality=4", async () => {
    const user = userEvent.setup();
    render(<FlashCardsBlock block={makeFlashBlock(singleCard)} moduleId="mod-1" />);

    // Flip the card first to reveal back
    const flipCard = screen.getByTestId("flash-card");
    await user.click(flipCard);

    // Click the Good button
    const goodBtn = screen.getByTestId("rate-Good");
    await user.click(goodBtn);

    expect(rateFlashCard).toHaveBeenCalledWith({
      blockId: "blk-fc-1",
      cardId: "fc-1",
      moduleId: "mod-1",
      quality: 4,
    });
  });

  it("flash_card_keyboard_space_flips — Space key toggles flip on focused card", async () => {
    const user = userEvent.setup();
    render(<FlashCardsBlock block={makeFlashBlock(singleCard)} moduleId="mod-1" />);

    const flipCard = screen.getByTestId("flash-card");
    flipCard.focus();

    expect(flipCard).not.toHaveClass("flipped");
    await user.keyboard(" ");
    expect(flipCard).toHaveClass("flipped");

    // Space again should unflip
    await user.keyboard(" ");
    expect(flipCard).not.toHaveClass("flipped");
  });
});
