import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { CalloutBlock } from "@/components/learning/CalloutBlock";
import type { ModuleBlock } from "@/types/learning";

function makeCalloutBlock(payloadJson: string): ModuleBlock {
  return {
    id: "blk-callout-1",
    moduleId: "mod-1",
    ordering: 2,
    blockType: "callout",
    status: "ready",
    paramsJson: '{"variant":"info"}',
    payloadJson,
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
  };
}

describe("CalloutBlock", () => {
  it("callout_renders_variant_styling — warning variant has distinct class from info", () => {
    const { rerender } = render(
      <CalloutBlock
        block={makeCalloutBlock('{"variant":"info","title":"Note","body":"Info content."}')}
      />
    );
    const infoBlock = screen.getByTestId("callout-block");
    expect(infoBlock).toHaveAttribute("data-variant", "info");
    expect(infoBlock.className).toContain("border-blue-400");

    rerender(
      <CalloutBlock
        block={makeCalloutBlock('{"variant":"warning","title":"Warning","body":"Be careful!"}')}
      />
    );
    const warningBlock = screen.getByTestId("callout-block");
    expect(warningBlock).toHaveAttribute("data-variant", "warning");
    expect(warningBlock.className).toContain("border-yellow-400");
  });

  it("callout_renders_title_and_body — title and body text are rendered", () => {
    render(
      <CalloutBlock
        block={makeCalloutBlock('{"variant":"success","title":"Great job!","body":"You passed the quiz."}')}
      />
    );
    expect(screen.getByText("Great job!")).toBeInTheDocument();
    expect(screen.getByText("You passed the quiz.")).toBeInTheDocument();
  });

  it("callout_no_title_renders_body_only — no title element when title is empty", () => {
    render(
      <CalloutBlock
        block={makeCalloutBlock('{"variant":"example","title":"","body":"An example here."}')}
      />
    );
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getByText("An example here.")).toBeInTheDocument();
  });
});
