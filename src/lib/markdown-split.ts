/**
 * splitMarkdownForInsert — split a markdown string into two halves at a natural
 * paragraph boundary near the 50% character midpoint.
 *
 * Algorithm:
 * 1. Split the markdown into "blocks" (double-newline-separated chunks).
 * 2. If the content is too short (< 400 chars or < 2 blocks) return
 *    [content, ""] — caller renders the slot AFTER the full content.
 * 3. Walk blocks accumulating character lengths. Find the block index where
 *    cumulative length first reaches or exceeds 50% of the total.
 * 4. If a Markdown heading block (starting with "#") exists within a ±2 block
 *    window around that midpoint, prefer to split immediately BEFORE that
 *    heading so the video lands at a natural section break.
 * 5. Guarantee both halves are non-empty: if the split index is 0 or equals
 *    the last block, fall back to [content, ""].
 *
 * Returns [firstHalf, secondHalf].  secondHalf may be an empty string when
 * the content is not worth splitting.
 */
export function splitMarkdownForInsert(markdown: string): [string, string] {
  if (!markdown) return ["", ""];

  // Split on one or more blank lines (handles \r\n too)
  const blocks = markdown.split(/\n\n+/);

  const totalChars = markdown.length;
  const tooShort = totalChars < 400 || blocks.length < 2;
  if (tooShort) return [markdown, ""];

  // Find the block index where cumulative char count reaches ~50%.
  const target = totalChars / 2;
  let cumulative = 0;
  let midIndex = 0; // index of the FIRST block in the second half

  for (let i = 0; i < blocks.length; i++) {
    cumulative += blocks[i].length + 2; // +2 for the \n\n separator
    if (cumulative >= target) {
      // midIndex is i+1: blocks[0..i] go to first half, blocks[i+1..] go second
      midIndex = i + 1;
      break;
    }
  }

  // If midIndex ended up beyond the last block, fall back to no-split
  if (midIndex <= 0 || midIndex >= blocks.length) {
    return [markdown, ""];
  }

  // Look for a heading within a ±2 block window around midIndex.
  // We want to split BEFORE the heading, so search blocks at indices
  // [midIndex - 2 .. midIndex + 2] for the first heading.
  const windowStart = Math.max(1, midIndex - 2); // never use index 0 (keeps first half non-empty)
  const windowEnd = Math.min(blocks.length - 1, midIndex + 2);

  let headingIndex = -1;
  for (let i = windowStart; i <= windowEnd; i++) {
    if (/^#{1,6}\s/.test(blocks[i])) {
      headingIndex = i;
      break;
    }
  }

  const splitAt = headingIndex >= 1 ? headingIndex : midIndex;

  // Guarantee both halves non-empty
  if (splitAt <= 0 || splitAt >= blocks.length) {
    return [markdown, ""];
  }

  const firstHalf = blocks.slice(0, splitAt).join("\n\n");
  const secondHalf = blocks.slice(splitAt).join("\n\n");

  return [firstHalf, secondHalf];
}
