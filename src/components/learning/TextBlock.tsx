import type { ModuleBlock, TextPayload } from "@/types/learning";
import { MarkdownRenderer } from "./MarkdownRenderer";

interface TextBlockProps {
  block: ModuleBlock;
}

/**
 * Renders a short-form text block (100-400 words) via MarkdownRenderer.
 * No "Mark complete" button — text blocks are informational only.
 * Glassmorphism container with prose styling.
 */
export function TextBlock({ block }: TextBlockProps) {
  let payload: TextPayload;
  try {
    payload = JSON.parse(block.payloadJson) as TextPayload;
  } catch {
    payload = { markdown: "Content unavailable." };
  }

  return (
    <div className="prose prose-invert max-w-none my-4" data-testid="text-block">
      <MarkdownRenderer content={payload.markdown} />
    </div>
  );
}
