import type { BlockStatus } from "@/types/learning";

interface BlockSkeletonProps {
  status: BlockStatus;
  onRetry?: () => void;
}

/**
 * Skeleton state for a block that is pending/generating/failed.
 * Wave 3 (03-05 Task 2) implements skeleton paragraphs + "Generating..." chip + retry card.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function BlockSkeleton({ status: _status, onRetry: _onRetry }: BlockSkeletonProps) {
  return <div data-testid="placeholder-block-skeleton">Not implemented</div>;
}
