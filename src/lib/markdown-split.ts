/**
 * splitMarkdownForInsert — split a markdown string into two halves near the
 * EARLY portion of the content (~first fifth) at a natural paragraph boundary,
 * so an injected element (the reference video) sits one or two scrolls down
 * from the lesson start rather than buried in the exact middle.
 *
 * Algorithm:
 * 1. Split the markdown into "blocks" (double-newline-separated chunks).
 * 2. If the content is too short (< 400 chars or < 2 blocks) return
 *    [content, ""] — caller renders the slot AFTER the full content.
 * 3. Walk blocks accumulating character lengths. Find the block index where
 *    cumulative length first reaches or exceeds ~20% of the total (the "early"
 *    target) — after the opening intro but well before the middle.
 * 4. If a Markdown heading block (starting with "#") exists within a ±2 block
 *    window around that point, prefer to split immediately BEFORE that heading
 *    so the video lands at a natural section break.
 * 5. Guarantee both halves are non-empty: if the split index is 0 or equals
 *    the last block, fall back to [content, ""].
 *
 * Returns [firstHalf, secondHalf].  secondHalf may be an empty string when
 * the content is not worth splitting.
 */
// Fraction of the lesson content to keep ABOVE the injected reference video.
// ~0.2 places it one or two scrolls from the lesson start (past the intro,
// well before the middle) so learners find it early without hunting.
const EARLY_SPLIT_FRACTION = 0.2;

// Minimum fraction of total chars the first half must contain before we accept
// a split (WR-04). Guards against landing the video after just ONE short
// opening block (e.g. a lone heading + a 30-char sentence) — the video would
// sit almost at the very top, not "past the intro". If the natural ~20% split
// leaves the first half below this floor we walk forward to accumulate a real
// intro, but never past MAX_FIRST_HALF_FRACTION so we don't drift to the mid.
const MIN_FIRST_HALF_FRACTION = 0.15;

// Hard ceiling on how far forward we will push the split while satisfying the
// minimum-intro floor. Kept clearly below the 50% midpoint so the video stays
// "early" while still allowing one substantial body block into the intro when
// the natural boundary would otherwise leave only a tiny opening block.
const MAX_FIRST_HALF_FRACTION = 0.35;

export function splitMarkdownForInsert(markdown: string): [string, string] {
  if (!markdown) return ["", ""];

  // Split on one or more blank lines (handles \r\n too)
  const blocks = markdown.split(/\n\n+/);

  const totalChars = markdown.length;
  const tooShort = totalChars < 400 || blocks.length < 2;
  if (tooShort) return [markdown, ""];

  // Find the block index where cumulative char count reaches the EARLY target
  // (~20%), so the video sits high up rather than at the 50% midpoint.
  const target = totalChars * EARLY_SPLIT_FRACTION;
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

  // WR-04: enforce a minimum intro. Length of blocks[0..at] joined.
  const firstHalfLen = (at: number) => blocks.slice(0, at).join("\n\n").length;

  const minFirst = totalChars * MIN_FIRST_HALF_FRACTION;
  const maxFirst = totalChars * MAX_FIRST_HALF_FRACTION;

  // Prefer the heading boundary ONLY if it leaves a real intro above the video.
  // The naive heading-preference could pull the split all the way back to an
  // early heading (e.g. block index 1), placing the video after just a single
  // tiny opening block — almost at the very top. When the heading split's first
  // half is below the intro floor, fall back to the ~20% target boundary, which
  // by construction sits past the intro.
  let splitAt = midIndex;
  if (headingIndex >= 1 && firstHalfLen(headingIndex) >= minFirst) {
    splitAt = headingIndex;
  }

  // Guarantee both halves non-empty
  if (splitAt <= 0 || splitAt >= blocks.length) {
    return [markdown, ""];
  }

  // If the chosen split STILL leaves too little intro above the video, walk the
  // split point forward (one block at a time) until the first half clears the
  // floor — but never past the max ceiling and never onto the last block (keeps
  // the second half non-empty). This keeps the video AFTER a reasonable intro
  // rather than immediately after a single short opening block.
  while (
    firstHalfLen(splitAt) < minFirst &&
    splitAt + 1 < blocks.length &&
    firstHalfLen(splitAt + 1) <= maxFirst
  ) {
    splitAt += 1;
  }

  // If we still can't reach the minimum intro without exceeding the ceiling or
  // consuming the whole content, the piece isn't worth splitting — render the
  // slot after the full content instead.
  if (firstHalfLen(splitAt) < minFirst || splitAt >= blocks.length) {
    return [markdown, ""];
  }

  const firstHalf = blocks.slice(0, splitAt).join("\n\n");
  const secondHalf = blocks.slice(splitAt).join("\n\n");

  return [firstHalf, secondHalf];
}
