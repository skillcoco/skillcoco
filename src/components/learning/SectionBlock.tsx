import type { ModuleBlock } from "@/types/learning";

interface SectionBlockProps {
  block: ModuleBlock;
  lessonIndex?: number;
  priorCompletedCount?: number;
  onMarkComplete?: (blockId: string) => void;
}

/**
 * Renders a section block: markdown content, optional skip-ahead banner,
 * and a "Mark complete" button.
 * Wave 3 (03-05 Task 2) implements the real component.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function SectionBlock({
  block: _block,
  lessonIndex: _lessonIndex,
  priorCompletedCount: _priorCompletedCount,
  onMarkComplete: _onMarkComplete,
}: SectionBlockProps) {
  return <div data-testid="placeholder-section-block">Not implemented</div>;
}
