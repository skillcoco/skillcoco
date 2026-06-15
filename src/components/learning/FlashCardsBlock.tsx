import { useState, useEffect, useRef } from "react";
import type { ModuleBlock, FlashCardsPayload, FlashCard } from "@/types/learning";
import { rateFlashCard } from "@/lib/tauri-commands";

interface FlashCardsBlockProps {
  block: ModuleBlock;
  moduleId: string;
  /**
   * Phase 4 (04-05) — optional block-completion signal. Fires ONCE when
   * every card in the block has been rated at least once in this session.
   * ModuleView callers pass nothing and the prop has zero behavioral effect.
   */
  onComplete?: () => void;
}

interface FlashCardProps {
  card: FlashCard;
  blockId: string;
  moduleId: string;
  /** Parent-supplied — called after a rating IPC resolves. */
  onRated?: (cardId: string) => void;
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
function FlashCardItem({ card, blockId, moduleId, onRated }: FlashCardProps) {
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
      // Phase 4 (04-05) — notify parent so it can aggregate
      // ratedCards for the daily-challenge completion signal.
      onRated?.(card.id);
    } catch (err) {
      console.error("rateFlashCard failed:", err);
      // Do NOT notify on failure — a failed IPC means the rating did not
      // persist server-side; treating it as "rated" would let an unrated
      // card count toward daily completion.
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
export function FlashCardsBlock({ block, moduleId, onComplete }: FlashCardsBlockProps) {
  let payload: FlashCardsPayload;
  try {
    payload = JSON.parse(block.payloadJson) as FlashCardsPayload;
  } catch {
    payload = { cards: [] };
  }

  // Phase 4 (04-05) — track which cards have been rated this session and
  // fire onComplete once every card has rated at least once.
  // completionFiredRef guards against double-fire on re-renders (e.g. if
  // the parent re-renders for unrelated reasons after the threshold lands).
  const [ratedCards, setRatedCards] = useState<Set<string>>(new Set());
  const completionFiredRef = useRef(false);

  const handleRated = (cardId: string) => {
    setRatedCards((prev) => {
      if (prev.has(cardId)) return prev;
      const next = new Set(prev);
      next.add(cardId);
      return next;
    });
  };

  useEffect(() => {
    if (
      onComplete &&
      payload.cards.length > 0 &&
      ratedCards.size >= payload.cards.length &&
      !completionFiredRef.current
    ) {
      completionFiredRef.current = true;
      onComplete();
    }
  }, [ratedCards, payload.cards.length, onComplete]);

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
          onRated={handleRated}
        />
      ))}
    </div>
  );
}
