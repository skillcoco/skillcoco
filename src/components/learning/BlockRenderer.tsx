import type { ModuleBlock } from "@/types/learning";
import { BlockSkeleton } from "./BlockSkeleton";
import { SectionBlock } from "./SectionBlock";
import { TextBlock } from "./TextBlock";
import { CalloutBlock } from "./CalloutBlock";
import { QuizBlock } from "./QuizBlock";
import { FlashCardsBlock } from "./FlashCardsBlock";
import { LabBlock } from "@/components/labs/LabBlock";
import { useLearningStore } from "@/stores/useLearningStore";

interface BlockRendererProps {
  block: ModuleBlock;
  moduleId: string;
  lessonIndex?: number;
  priorCompletedCount?: number;
  /** Reserved for future use — passed through to QuizBlock when Quiz support lands (03-06). */
  trackId?: string;
}

/**
 * Discriminated-union renderer for Module blocks.
 *
 * Non-ready blocks (pending | generating | failed) are always rendered as
 * BlockSkeleton regardless of blockType. This ensures skeleton / retry UI
 * is shown during generation or on failure.
 *
 * Ready blocks are dispatched by blockType:
 *   section    -> SectionBlock  (markdown + mark-complete)
 *   text       -> TextBlock     (markdown only, no mark-complete)
 *   callout    -> CalloutBlock  (variant-styled callout box)
 *   quiz       -> QuizBlock     (MCQ flow — Wave 4)
 *   flash_cards -> FlashCardsBlock (flip cards — Wave 3)
 *
 * Unknown blockTypes render a fallback div with a descriptive message.
 */
export function BlockRenderer({
  block,
  moduleId,
  lessonIndex,
  priorCompletedCount,
  trackId,
}: BlockRendererProps) {
  const regenerateLesson = useLearningStore((s) => s.regenerateLesson);

  // Non-ready blocks: always show skeleton / retry card
  if (
    block.status === "pending" ||
    block.status === "generating" ||
    block.status === "failed"
  ) {
    return (
      <BlockSkeleton
        status={block.status}
        onRetry={
          block.status === "failed"
            ? () => regenerateLesson(block.id)
            : undefined
        }
      />
    );
  }

  // Ready: dispatch by blockType
  switch (block.blockType) {
    case "section":
      return (
        <SectionBlock
          block={block}
          moduleId={moduleId}
          lessonIndex={lessonIndex}
          priorCompletedCount={priorCompletedCount}
        />
      );
    case "text":
      return <TextBlock block={block} />;
    case "callout":
      return <CalloutBlock block={block} />;
    case "quiz":
      return <QuizBlock block={block} moduleId={moduleId} trackId={trackId} />;
    case "flash_cards":
      return (
        <FlashCardsBlock
          block={block}
          moduleId={moduleId}
        />
      );
    case "lab":
      return <LabBlock block={block} trackId={trackId} />;
    default:
      return (
        <div data-testid="unsupported-block">
          Unsupported block type: {block.blockType}
        </div>
      );
  }
}
