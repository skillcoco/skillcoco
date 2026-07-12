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

const { redeemLicenseMock, downloadAndImportPackMock, recoverRedeemedPackMock } =
  vi.hoisted(() => ({
    redeemLicenseMock: vi.fn(),
    downloadAndImportPackMock: vi.fn(),
    recoverRedeemedPackMock: vi.fn(),
  }));

vi.mock("@/lib/tauri-commands", () => ({
  redeemLicense: redeemLicenseMock,
  downloadAndImportPack: downloadAndImportPackMock,
  recoverRedeemedPack: recoverRedeemedPackMock,
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
  // CR-01 default: no local recovery available unless a test says so.
  recoverRedeemedPackMock.mockResolvedValue(null);
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

  // WR-06 — classification happens EXCLUSIVELY on the structured `kind`
  // field the backend's RedeemIpcError serializes ({ kind, message });
  // the old Display-copy regex fallback is deleted. All four contract
  // taxonomy variants must route through `kind`.
  it.each([
    ["invalid_key", /this license key isn't valid\. check for typos/i],
    ["already_redeemed", /this license key has already been redeemed\./i],
    ["revoked", /this license key has been revoked\./i],
    ["issuer_unreachable", /couldn't reach the license server\./i],
  ])(
    "WR-06: classifies { kind: %s } via the structured field and renders its locked copy",
    async (kind, expectedCopy) => {
      redeemLicenseMock.mockRejectedValue({
        kind,
        message: "diagnostic detail — never rendered",
      });

      render(<RedeemLicenseFlow />);

      await userEvent.type(screen.getByLabelText(/license key/i), "SOME-KEY");
      await userEvent.click(
        screen.getByRole("button", { name: /^redeem$/i }),
      );

      await waitFor(() => {
        expect(screen.getByText(expectedCopy)).toBeInTheDocument();
      });
      // The raw diagnostic message is NEVER rendered (T-15-16).
      expect(
        screen.queryByText(/diagnostic detail/i),
      ).not.toBeInTheDocument();
    },
  );

  it("WR-06: a free-text error mentioning 'connection' renders the GENERIC copy — no substring matching on human copy", async () => {
    redeemLicenseMock.mockRejectedValue(
      new Error("Redeem request failed: connection closed before message completed"),
    );

    render(<RedeemLicenseFlow />);

    await userEvent.type(screen.getByLabelText(/license key/i), "SOME-KEY");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/something went wrong redeeming this key\./i),
      ).toBeInTheDocument();
    });
    // Must NOT be misclassified as issuer_unreachable (no Retry button).
    expect(
      screen.queryByRole("button", { name: /^retry$/i }),
    ).not.toBeInTheDocument();
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

describe("RedeemLicenseFlow — CR-01 stranded-purchase recovery", () => {
  it("retries a failed confirm-stage download with the HELD downloadUrl — redeemLicense is never re-called (the key is already burned)", async () => {
    redeemLicenseMock.mockResolvedValue(successfulRedeemResult());
    downloadAndImportPackMock
      .mockRejectedValueOnce({
        kind: "issuer_unreachable",
        message: "connect timeout",
      })
      .mockResolvedValueOnce({ trackId: "trk-recovered" });

    render(<RedeemLicenseFlow />);

    await userEvent.type(screen.getByLabelText(/license key/i), "ABCD-1234");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /confirm & download/i }),
      ).toBeInTheDocument();
    });
    await userEvent.click(
      screen.getByRole("button", { name: /confirm & download/i }),
    );

    // Download failed — a Retry affordance must exist.
    const retryButton = await screen.findByRole("button", {
      name: /^retry$/i,
    });
    await userEvent.click(retryButton);

    await waitFor(() => {
      expect(
        screen.getByText(/course imported\. it's now available/i),
      ).toBeInTheDocument();
    });

    // The single-use key was redeemed exactly ONCE; the retry re-used the
    // in-memory download URL instead of re-calling redeemLicense (which
    // would dead-end on already_redeemed).
    expect(redeemLicenseMock).toHaveBeenCalledTimes(1);
    expect(downloadAndImportPackMock).toHaveBeenCalledTimes(2);
    expect(downloadAndImportPackMock.mock.calls[1][0].downloadUrl).toBe(
      downloadAndImportPackMock.mock.calls[0][0].downloadUrl,
    );
  });

  it("on already_redeemed, recovers locally when this device already holds the pack (no dead end)", async () => {
    const onImported = vi.fn();
    redeemLicenseMock.mockRejectedValue({
      kind: "already_redeemed",
      message: "key consumed",
    });
    recoverRedeemedPackMock.mockResolvedValue({
      trackId: "trk-local",
      alreadyImported: true,
    });

    render(<RedeemLicenseFlow onImported={onImported} />);

    await userEvent.type(screen.getByLabelText(/license key/i), "USED-KEY-1");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/already redeemed on this device.*in your library/i),
      ).toBeInTheDocument();
    });
    expect(recoverRedeemedPackMock).toHaveBeenCalledWith("USED-KEY-1");
    expect(onImported).toHaveBeenCalledWith("trk-local");
    // NOT rendered as an error dead end.
    expect(
      screen.queryByText(/contact your course provider/i),
    ).not.toBeInTheDocument();
  });

  it("on already_redeemed with NO local recovery, renders issuer-contact guidance with an order reference pointer", async () => {
    redeemLicenseMock.mockRejectedValue({
      kind: "already_redeemed",
      message: "key consumed",
    });
    recoverRedeemedPackMock.mockResolvedValue(null);

    render(<RedeemLicenseFlow />);

    await userEvent.type(screen.getByLabelText(/license key/i), "USED-KEY-2");
    await userEvent.click(screen.getByRole("button", { name: /^redeem$/i }));

    await waitFor(() => {
      expect(
        screen.getByText(
          /already been redeemed.*contact your course provider.*order reference/i,
        ),
      ).toBeInTheDocument();
    });
  });
});
