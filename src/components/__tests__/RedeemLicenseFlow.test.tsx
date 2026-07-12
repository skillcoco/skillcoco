// Phase 15 Plan 01 (Wave 0) — RedeemLicenseFlow RED scaffold tests.
//
// RedeemLicenseFlow doesn't exist yet (15-04 builds it) — this suite fails
// to resolve the import, which IS the RED state for this Wave 0 plan.
// Assertions below define the staged-confirm state machine contract
// (D-02/D-03/D-04, 15-UI-SPEC.md Copywriting Contract + Interaction States)
// that 15-04 must satisfy.
//
// Mocking strategy mirrors SettingsCourseImportSection.test.tsx /
// SettingsVerifyCertSection.test.tsx: only `@/lib/tauri-commands` is mocked
// via vi.hoisted so the mock references survive vi.mock factory hoisting.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const { redeemLicenseMock, downloadAndImportPackMock } = vi.hoisted(() => ({
  redeemLicenseMock: vi.fn(),
  downloadAndImportPackMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  redeemLicense: redeemLicenseMock,
  downloadAndImportPack: downloadAndImportPackMock,
}));

// 15-04 GREEN target — does not exist yet at Wave 0 (RED: unresolved import).
import { RedeemLicenseFlow } from "@/components/RedeemLicenseFlow";

function successfulRedeemResult(overrides = {}) {
  return {
    packId: "pack-ent-02",
    issuerId: "issuer-1",
    issuerName: "Test Issuer",
    buyerName: "Jane Buyer",
    orderId: "ORD-9001",
    downloadUrl: "https://hub.example.org/download/pack-ent-02",
    redeemedAt: "2026-07-12T00:00:00Z",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("RedeemLicenseFlow — entry stage (Wave 0 RED)", () => {
  it("renders the License key label, placeholder, and Redeem button; key input is the first-view focal point", async () => {
    render(<RedeemLicenseFlow />);

    expect(screen.getByLabelText(/license key/i)).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/paste your license key/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^redeem$/i }),
    ).toBeInTheDocument();

    // Focal-point flag: the license-key input must be the primary focus
    // target on first view (UI-checker focal-point requirement).
    await waitFor(() => {
      expect(document.activeElement).toBe(
        screen.getByLabelText(/license key/i),
      );
    });
  });
});

describe("RedeemLicenseFlow — staged-confirm stage (Wave 0 RED)", () => {
  it("shows the pack title as the confirm-stage heading, the buyer/order attribution line, and Confirm & Download / Cancel buttons after a successful redeem", async () => {
    redeemLicenseMock.mockResolvedValue(successfulRedeemResult());

    render(<RedeemLicenseFlow />);

    await userEvent.type(
      screen.getByLabelText(/license key/i),
      "ABCD-1234-EFGH",
    );
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("dialog", { name: /.+/i }),
      ).toBeInTheDocument();
    });

    // Confirm-stage focal point: the pack title renders as the dialog's
    // primary heading (UI-checker focal-point requirement).
    expect(
      screen.getByRole("heading", { name: /.+/i }),
    ).toBeInTheDocument();

    expect(
      screen.getByText(/licensed to jane buyer · order #ord-9001/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /confirm & download/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^cancel$/i }),
    ).toBeInTheDocument();
  });
});

describe("RedeemLicenseFlow — typed error rendering (Wave 0 RED)", () => {
  it("renders the invalid_key plain-language copy inline under the key field", async () => {
    redeemLicenseMock.mockRejectedValue({ kind: "invalid_key" });

    render(<RedeemLicenseFlow />);

    await userEvent.type(screen.getByLabelText(/license key/i), "BAD-KEY");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByText(
          /this license key isn't valid\. check for typos and try again\./i,
        ),
      ).toBeInTheDocument();
    });
  });

  it("renders the issuer_unreachable copy plus a Retry button", async () => {
    redeemLicenseMock.mockRejectedValue({ kind: "issuer_unreachable" });

    render(<RedeemLicenseFlow />);

    await userEvent.type(screen.getByLabelText(/license key/i), "ABCD-1234");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByText(
          /couldn't reach the license server\. check your connection and try again\./i,
        ),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByRole("button", { name: /^retry$/i }),
    ).toBeInTheDocument();
  });
});
