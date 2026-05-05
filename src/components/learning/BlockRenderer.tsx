import type { ModuleBlock } from "@/types/learning";

interface BlockRendererProps {
  block: ModuleBlock;
}

/**
 * Discriminated-union renderer for Module blocks.
 * Wave 3 (03-05 Task 2) implements the real switch on blockType.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function BlockRenderer({ block: _block }: BlockRendererProps) {
  return <div data-testid="placeholder-block-renderer">Not implemented</div>;
}
