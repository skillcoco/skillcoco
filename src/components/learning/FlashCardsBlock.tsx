import { useState } from "react";
import type { ModuleBlock, FlashCardsPayload, FlashCard } from "@/types/learning";
import { rateFlashCard } from "@/lib/tauri-commands";

interface FlashCardsBlockProps {
  block: ModuleBlock;
  moduleId: string;
}

interface FlashCardProps {
  card: FlashCard;
  blockId: string;
  moduleId: string;
}

/**
 * Single flip card with CSS 3D transform.
 * State: `flipped` boolean — toggled by click or Space/Enter key.
 * Back side shows quality rating buttons (Hard/Good/Easy).
 * Rating calls rateFlashCard IPC — click propagation stopped so button
 * click does not also toggle the flip.
 *
 * CSS classes: flip-card, flip-card-inner, flip-card-front, flip-card-back
 * (defined in src/index.css, no library needed).
 */
function FlashCardItem({ card, blockId, moduleId }: FlashCardProps) {
  const [flipped, setFlipped] = useState(false);

  function toggle() {
    setFlipped((f) => !f);
  }

  async function rate(quality: number, e: React.MouseEvent) {
    e.stopPropagation(); // prevent button click from toggling flip
    try {
      await rateFlashCard({
        blockId,
        cardId: card.id,
        moduleId,
        quality,
      });
    } catch (err) {
      console.error("rateFlashCard failed:", err);
    }
  }

  return (
    <div
      className={`flip-card glass rounded-lg cursor-pointer min-h-[200px]${flipped ? " flipped" : ""}`}
      data-flipped={flipped}
      data-testid="flash-card"
      onClick={toggle}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          toggle();
        }
      }}
      tabIndex={0}
      role="button"
      aria-label={flipped ? "Flash card (showing answer)" : "Flash card (click to reveal)"}
    >
      <div className="flip-card-inner w-full h-full min-h-[200px]">
        {/* Front */}
        <div className="flip-card-front p-6 flex flex-col justify-between min-h-[200px]">
          <p className="text-sm font-medium text-foreground">{card.front}</p>
          <p className="text-xs text-foreground/50 mt-4">Click to reveal</p>
        </div>

        {/* Back */}
        <div className="flip-card-back p-6 flex flex-col justify-between min-h-[200px]">
          <p className="text-sm text-foreground/90">{card.back}</p>
          <div className="flex gap-2 mt-4" onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              className="glass-strong px-3 py-1.5 rounded text-xs font-medium hover:opacity-90 transition-opacity"
              data-testid="rate-Hard"
              onClick={(e) => rate(2, e)}
            >
              Hard
            </button>
            <button
              type="button"
              className="glass-strong px-3 py-1.5 rounded text-xs font-medium hover:opacity-90 transition-opacity"
              data-testid="rate-Good"
              onClick={(e) => rate(4, e)}
            >
              Good
            </button>
            <button
              type="button"
              className="glass-strong px-3 py-1.5 rounded text-xs font-medium hover:opacity-90 transition-opacity"
              data-testid="rate-Easy"
              onClick={(e) => rate(5, e)}
            >
              Easy
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Renders 1-3 inline flip cards for a flash_cards block.
 *
 * Parses payloadJson to get the cards array (FlashCardsPayload.cards).
 * Each card is rendered in a 2-column grid on md+ screens.
 * Rating buttons call rateFlashCard IPC directly (no store action needed here).
 *
 * CSS flip animation lives in index.css (Phase 3).
 * No emojis; uses glass/glass-strong utility classes.
 */
export function FlashCardsBlock({ block, moduleId }: FlashCardsBlockProps) {
  let payload: FlashCardsPayload;
  try {
    payload = JSON.parse(block.payloadJson) as FlashCardsPayload;
  } catch {
    payload = { cards: [] };
  }

  if (!payload.cards || payload.cards.length === 0) {
    return (
      <div className="glass rounded-lg p-6 my-6 text-sm text-foreground/60">
        No flash cards available for this lesson.
      </div>
    );
  }

  return (
    <div className="my-6 grid gap-4 md:grid-cols-2">
      {payload.cards.map((card) => (
        <FlashCardItem
          key={card.id}
          card={card}
          blockId={block.id}
          moduleId={moduleId}
        />
      ))}
    </div>
  );
}
