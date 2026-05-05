import type { BlockStatus } from "@/types/learning";

interface BlockSkeletonProps {
  status: BlockStatus;
  onRetry?: () => void;
}

/**
 * Skeleton state for a block that is pending, generating, or failed.
 *
 * - pending / generating: skeleton paragraphs + "Generating..." chip
 * - failed: "Couldn't generate" inline card with Retry button
 *
 * Glassmorphism styling (glass/glass-strong) per Phase 3 CONTEXT.md.
 * No emojis — icons use text or Lucide (none used here to keep deps minimal).
 */
export function BlockSkeleton({ status, onRetry }: BlockSkeletonProps) {
  if (status === "failed") {
    return (
      <div
        className="glass rounded-lg p-6 my-4 border-l-4 border-red-400"
        data-testid="block-retry-card"
      >
        <p className="text-sm text-foreground/80 mb-3">
          Couldn't generate this lesson. The AI provider may be rate-limited.
        </p>
        {onRetry && (
          <button
            type="button"
            className="glass-strong px-4 py-2 rounded-md text-sm font-medium hover:opacity-90 transition-opacity"
            onClick={onRetry}
          >
            Retry
          </button>
        )}
      </div>
    );
  }

  // pending | generating
  return (
    <div
      className="glass rounded-lg p-6 my-4"
      data-testid="block-skeleton"
    >
      <div className="flex items-center gap-2 mb-4">
        <span className="inline-block h-2 w-2 rounded-full bg-blue-400 animate-pulse" />
        <span className="text-xs text-foreground/60">Generating...</span>
      </div>
      <div className="space-y-3">
        <div className="h-3 bg-foreground/10 rounded animate-pulse w-3/4" />
        <div className="h-3 bg-foreground/10 rounded animate-pulse w-full" />
        <div className="h-3 bg-foreground/10 rounded animate-pulse w-5/6" />
      </div>
    </div>
  );
}
