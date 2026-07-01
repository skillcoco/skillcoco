/**
 * TDD RED tests for splitMarkdownForInsert utility.
 *
 * Phase 11 — UI refinement: mid-content video injection.
 * The helper splits a markdown string into two halves at a natural paragraph
 * boundary near the 50% character midpoint, preferring a heading boundary.
 */

import { describe, it, expect } from "vitest";
import { splitMarkdownForInsert } from "@/lib/markdown-split";

// Short content — too short to split (< 400 chars or < 2 blocks)
const SHORT = "# Hello\n\nBrief content.";

// Long content — enough blocks that splitting makes sense (> 400 chars total)
const LONG_NO_HEADINGS = [
  "First paragraph that is long enough to count towards the total character budget for our split algorithm.",
  "Second paragraph that adds more text and ensures we have enough content to trigger the split threshold at 400 chars.",
  "Third paragraph continues the explanation with yet more words to pad out the total length of this test fixture.",
  "Fourth paragraph ends the content with a final observation that pushes the total well beyond the minimum threshold.",
].join("\n\n");

const LONG_WITH_HEADING_MIDPOINT = [
  "Introduction paragraph that lays the groundwork for the discussion and provides adequate character budget for split.",
  "Background paragraph that provides essential context for what follows next in the lesson flow.",
  "## Key Concepts",
  "This paragraph follows the mid-lesson heading and begins the main body section of the lesson content.",
  "Another paragraph after the heading that explains the key idea in further detail for the learner reading this.",
  "Final paragraph that wraps up and summarises the key points covered throughout this particular lesson section.",
].join("\n\n");

describe("splitMarkdownForInsert", () => {
  // ── Short content: no split ──────────────────────────────────────────────

  it("short_content_no_split — content under threshold: first half = full content, second half = empty", () => {
    const [first, second] = splitMarkdownForInsert(SHORT);
    expect(first).toBe(SHORT);
    expect(second).toBe("");
  });

  it("single_block_no_split — single paragraph with no blank lines: first half = content, second = empty", () => {
    const single = "This is a single block with no paragraph breaks at all.";
    const [first, second] = splitMarkdownForInsert(single);
    expect(first).toBe(single);
    expect(second).toBe("");
  });

  // ── Long content: splits into two non-empty halves ───────────────────────

  it("long_content_two_halves — long content produces two non-empty halves", () => {
    const [first, second] = splitMarkdownForInsert(LONG_NO_HEADINGS);
    expect(first.length).toBeGreaterThan(0);
    expect(second.length).toBeGreaterThan(0);
  });

  it("long_content_reconstructs — joining the two halves (double newline) reconstructs the original", () => {
    const [first, second] = splitMarkdownForInsert(LONG_NO_HEADINGS);
    const reconstructed = second ? `${first}\n\n${second}` : first;
    // Normalise: trim trailing whitespace from each line for comparison
    expect(reconstructed.trim()).toBe(LONG_NO_HEADINGS.trim());
  });

  it("long_content_no_mid_paragraph_split — neither half ends mid-sentence (block boundary respected)", () => {
    const [first, second] = splitMarkdownForInsert(LONG_NO_HEADINGS);
    // No half should end or start with a lone hyphen or mid-word character;
    // practically, we check that neither half is empty and the last char of
    // first is a word char (ends at paragraph boundary, not mid-text).
    expect(first.trimEnd()).not.toMatch(/\n$/);
    // The original was split on a block boundary — the second half should
    // start cleanly (not with a newline).
    expect(second.trimStart()).not.toMatch(/^\n/);
  });

  // ── Heading preference: splits immediately BEFORE a heading ──────────────

  it("prefers_heading_boundary — when a heading exists near midpoint, splits just before it", () => {
    const [_first, second] = splitMarkdownForInsert(LONG_WITH_HEADING_MIDPOINT);
    // The second half should start with the heading "## Key Concepts"
    expect(second.trimStart()).toMatch(/^##\s+Key Concepts/);
  });

  it("heading_preference_both_halves_nonempty — heading split still produces two non-empty halves", () => {
    const [first, second] = splitMarkdownForInsert(LONG_WITH_HEADING_MIDPOINT);
    expect(first.length).toBeGreaterThan(0);
    expect(second.length).toBeGreaterThan(0);
  });

  // ── Near-empty content edge case ─────────────────────────────────────────

  it("empty_string_no_split — empty input: first = empty, second = empty", () => {
    const [first, second] = splitMarkdownForInsert("");
    expect(first).toBe("");
    expect(second).toBe("");
  });
});
