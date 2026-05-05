import type { ModuleBlock } from "@/types/learning";

interface FlashCardsBlockProps {
  block: ModuleBlock;
  onRate?: (cardId: string, quality: number) => void;
}

/**
 * Inline flip-card UI for flash_cards blocks.
 * Wave 3 (03-05 Task 3) implements card flip and SM-2 rating.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function FlashCardsBlock({ block: _block, onRate: _onRate }: FlashCardsBlockProps) {
  return <div data-testid="placeholder-flash-cards-block">Not implemented</div>;
}
