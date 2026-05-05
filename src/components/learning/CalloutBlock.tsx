import type { ModuleBlock, CalloutPayload } from "@/types/learning";

interface CalloutBlockProps {
  block: ModuleBlock;
}

const VARIANT_STYLES: Record<string, string> = {
  info:    "border-l-4 border-blue-400",
  warning: "border-l-4 border-yellow-400",
  success: "border-l-4 border-green-400",
  example: "border-l-4 border-purple-400",
  code:    "border-l-4 border-gray-400",
  quote:   "border-l-4 border-foreground/20 italic",
};

/**
 * Renders a callout block with glassmorphism border variant styling.
 *
 * Variants: info | warning | success | example | code | quote
 * No emojis — uses border color accent only.
 * Accessible: data-variant attribute on root for test and CSS targeting.
 */
export function CalloutBlock({ block }: CalloutBlockProps) {
  let payload: CalloutPayload;
  try {
    payload = JSON.parse(block.payloadJson) as CalloutPayload;
  } catch {
    payload = { variant: "info", title: "", body: "Content unavailable." };
  }

  const variantClass = VARIANT_STYLES[payload.variant] ?? VARIANT_STYLES.info;

  return (
    <div
      className={`glass rounded-md p-4 my-4 ${variantClass}`}
      data-variant={payload.variant}
      data-testid="callout-block"
    >
      {payload.title && (
        <h4 className="text-sm font-semibold text-foreground mb-1">
          {payload.title}
        </h4>
      )}
      <p className="text-sm text-foreground/80 m-0">{payload.body}</p>
    </div>
  );
}
