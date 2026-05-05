import type { ModuleBlock } from "@/types/learning";

interface TextBlockProps {
  block: ModuleBlock;
}

/**
 * Renders a short-form text block.
 * Wave 3 (03-05 Task 2) implements the real component.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function TextBlock({ block: _block }: TextBlockProps) {
  return <div data-testid="placeholder-text-block">Not implemented</div>;
}
