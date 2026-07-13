// Phase 16 Plan 02 Task 1 — BuyerAttributionLine shared extract (D-07).
//
// This component is the D-07 "third call site" consolidation target: it
// centralizes the "Licensed to {buyerName} · order #{orderId}" copy that
// previously existed as inline JSX duplicated across SettingsCourseImportSection
// and RedeemLicenseFlow. Renders nothing when either prop is missing.

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { BuyerAttributionLine } from "@/components/BuyerAttributionLine";

describe("BuyerAttributionLine — Phase 16 Plan 02 Task 1", () => {
  it("renders the attribution line when both buyerName and orderId are present", () => {
    render(<BuyerAttributionLine buyerName="Jane Doe" orderId="ORD-123" />);
    expect(
      screen.getByText("Licensed to Jane Doe · order #ORD-123"),
    ).toBeInTheDocument();
  });

  it("renders nothing when buyerName is missing", () => {
    const { container } = render(<BuyerAttributionLine orderId="ORD-123" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when orderId is missing", () => {
    const { container } = render(<BuyerAttributionLine buyerName="Jane Doe" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when both props are missing", () => {
    const { container } = render(<BuyerAttributionLine />);
    expect(container).toBeEmptyDOMElement();
  });

  it("uses the locked text-xs text-muted-foreground styling", () => {
    render(<BuyerAttributionLine buyerName="Jane Doe" orderId="ORD-123" />);
    const el = screen.getByText("Licensed to Jane Doe · order #ORD-123");
    expect(el.className).toContain("text-xs");
    expect(el.className).toContain("text-muted-foreground");
  });
});
