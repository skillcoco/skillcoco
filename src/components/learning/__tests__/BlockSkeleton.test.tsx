import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { BlockSkeleton } from "@/components/learning/BlockSkeleton";

describe("BlockSkeleton Phase 3 scaffolds", () => {
  it("block_skeleton_shows_chip — generating status renders Generating... chip", () => {
    render(<BlockSkeleton status="generating" />);

    // FAILS in Wave 0: placeholder renders "placeholder-block-skeleton", not the chip.
    // GREEN in 03-05 Task 2 when skeleton paragraphs + chip are implemented.
    expect(screen.getByText(/generating/i)).toBeInTheDocument();
  });

  it("block_skeleton_retry_card — failed status renders Retry button", async () => {
    const onRetry = vi.fn();
    render(<BlockSkeleton status="failed" onRetry={onRetry} />);

    // FAILS in Wave 0: placeholder doesn't render a retry button.
    // GREEN in 03-05 Task 2.
    const retryBtn = screen.getByRole("button", { name: /retry/i });
    expect(retryBtn).toBeInTheDocument();

    await userEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalledOnce();
  });
});
