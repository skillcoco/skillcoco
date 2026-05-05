import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FlashCardsBlock } from "@/components/learning/FlashCardsBlock";
import type { ModuleBlock } from "@/types/learning";

const mockPayload = {
  cards: [
    {
      id: "fc-1",
      front: "What is a Pod?",
      back: "The smallest deployable unit in Kubernetes.",
    },
  ],
};

const mockBlock: ModuleBlock = {
  id: "blk-fc-1",
  moduleId: "mod-1",
  ordering: 5,
  blockType: "flash_cards",
  status: "ready",
  paramsJson: '{"card_count":1}',
  payloadJson: JSON.stringify(mockPayload),
  sourceAnchorsJson: "[]",
  metadataJson: '{"concept_id":null}',
  retryCount: 0,
  createdAt: "2026-05-05T00:00:00Z",
  updatedAt: "2026-05-05T00:00:00Z",
};

describe("FlashCardsBlock Phase 3 scaffolds", () => {
  it("flash_card_flip — click card front to reveal back (CSS flipped class applied)", async () => {
    const user = userEvent.setup();
    render(<FlashCardsBlock block={mockBlock} />);

    // FAILS in Wave 0: placeholder renders "Not implemented", no flip-card element.
    // GREEN in 03-05 Task 3 when flip-card CSS + click handler is implemented.
    const cardFront = screen.getByText("What is a Pod?");
    expect(cardFront).toBeInTheDocument();

    await user.click(cardFront.closest(".flip-card") ?? cardFront);

    // After click, the flip-card-inner should have the "flipped" class
    expect(cardFront.closest(".flip-card")).toHaveClass("flipped");
  });
});
