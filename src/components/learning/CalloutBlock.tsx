import type { ModuleBlock } from "@/types/learning";

interface CalloutBlockProps {
  block: ModuleBlock;
}

/**
 * Renders a callout block with glassmorphism variant styling.
 * Wave 3 (03-05 Task 2) implements the real component.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function CalloutBlock({ block: _block }: CalloutBlockProps) {
  return <div data-testid="placeholder-callout-block">Not implemented</div>;
}
