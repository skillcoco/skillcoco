import type { BlockStatus } from "@/types/learning";

interface BlockSkeletonProps {
  status: BlockStatus;
  onRetry?: () => void;
  /** Actual generator error from Rust (metadata_json.lastError). */
  errorMessage?: string;
}

/**
 * Skeleton state for a block that is pending, generating, or failed.
 *
 * - pending / generating: skeleton paragraphs + "Generating..." chip
 * - failed: "Couldn't generate" inline card with Retry button. Shows the
 *   actual generator error when one is captured in metadata_json.lastError;
 *   falls back to a generic message otherwise.
 *
 * Glassmorphism styling (glass/glass-strong) per Phase 3 CONTEXT.md.
 */
export function BlockSkeleton({ status, onRetry, errorMessage }: BlockSkeletonProps) {
  if (status === "failed") {
    return (
      <div
        className="glass rounded-lg p-6 my-4 border-l-4 border-red-400"
        data-testid="block-retry-card"
      >
        <p className="text-sm font-medium text-foreground/90 mb-2">
          Couldn't generate this block.
        </p>
        {errorMessage ? (
          <p
            className="text-xs text-foreground/70 mb-3 font-mono whitespace-pre-wrap break-words"
            data-testid="block-error-detail"
          >
            {errorMessage}
          </p>
        ) : (
          <p className="text-xs text-foreground/60 mb-3">
            No error detail captured. Check the dev-server console for `block ... generation failed:` logs.
          </p>
        )}
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
