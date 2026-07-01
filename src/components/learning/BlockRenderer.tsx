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
  /**
   * Phase 11 — optional mid-content slot forwarded to SectionBlock only.
   * Other block types (quiz, text, callout, lab) ignore this prop.
   */
  referenceSlot?: React.ReactNode;
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
  referenceSlot,
}: BlockRendererProps) {
  const regenerateLesson = useLearningStore((s) => s.regenerateLesson);

  // Non-ready blocks: always show skeleton / retry card
  if (
    block.status === "pending" ||
    block.status === "generating" ||
    block.status === "failed"
  ) {
    // Surface the real generator error when one was captured by Rust into
    // metadata_json.lastError (see update_block_failed_with_error in
    // commands/blocks.rs). Falls back to undefined → BlockSkeleton's generic
    // "no error detail captured" hint.
    let errorMessage: string | undefined;
    if (block.status === "failed" && block.metadataJson) {
      try {
        const meta = JSON.parse(block.metadataJson) as { lastError?: unknown };
        if (typeof meta.lastError === "string" && meta.lastError.length > 0) {
          errorMessage = meta.lastError;
        }
      } catch {
        // metadata_json malformed; fall back to generic message
      }
    }
    return (
      <BlockSkeleton
        status={block.status}
        errorMessage={errorMessage}
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
          referenceSlot={referenceSlot}
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
