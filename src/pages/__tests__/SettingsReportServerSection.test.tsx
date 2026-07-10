// Phase 18 (18-06 / D-13) — SettingsReportServerSection tests.
//
// Mocking strategy (mirrors SettingsVerifyCertSection.test.tsx): only
// `@/lib/tauri-commands` is mocked via vi.hoisted so the mock references
// survive vi.mock factory hoisting.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LearnerProfile } from "@/types/learning";

const { getOrCreateProfileMock, updateProfileMock, submitEvidenceReportMock } =
  vi.hoisted(() => ({
    getOrCreateProfileMock: vi.fn(),
    updateProfileMock: vi.fn(),
    submitEvidenceReportMock: vi.fn(),
  }));

vi.mock("@/lib/tauri-commands", () => ({
  getOrCreateProfile: getOrCreateProfileMock,
  updateProfile: updateProfileMock,
  submitEvidenceReport: submitEvidenceReportMock,
}));

import { SettingsReportServerSection } from "@/pages/SettingsReportServerSection";

function baseProfile(overrides: Partial<LearnerProfile> = {}): LearnerProfile {
  return {
    id: "lp1",
    displayName: "Ada",
    learningStyle: "visual",
    experienceLevel: "beginner",
    preferencesJson: "{}",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  getOrCreateProfileMock.mockResolvedValue(baseProfile());
  updateProfileMock.mockResolvedValue(baseProfile());
});

describe("SettingsReportServerSection — Phase 18 (18-06 / D-13)", () => {
  it("renders reportServerUrl as text and reportServerToken as a masked/password field", async () => {
    render(<SettingsReportServerSection />);

    await waitFor(() => {
      expect(getOrCreateProfileMock).toHaveBeenCalled();
    });

    const urlInput = screen.getByLabelText(/report server url/i);
    const tokenInput = screen.getByLabelText(/report server token/i);

    expect(urlInput).toHaveAttribute("type", "text");
    expect(tokenInput).toHaveAttribute("type", "password");
  });

  it("hydrates persisted reportServerUrl/reportServerToken from preferences_json on mount", async () => {
    getOrCreateProfileMock.mockResolvedValue(
      baseProfile({
        preferencesJson: JSON.stringify({
          reportServerUrl: "https://reports.example.org",
          reportServerToken: "secret-tok",
        }),
      }),
    );

    render(<SettingsReportServerSection />);

    await waitFor(() => {
      expect(screen.getByLabelText(/report server url/i)).toHaveValue(
        "https://reports.example.org",
      );
    });
    expect(screen.getByLabelText(/report server token/i)).toHaveValue(
      "secret-tok",
    );
  });

  it("saves reportServerUrl/reportServerToken via updateProfile preferences_json merge", async () => {
    render(<SettingsReportServerSection />);
    await waitFor(() => {
      expect(getOrCreateProfileMock).toHaveBeenCalled();
    });

    await userEvent.type(
      screen.getByLabelText(/report server url/i),
      "https://hub.example.org",
    );
    await userEvent.type(
      screen.getByLabelText(/report server token/i),
      "tok123",
    );
    await userEvent.click(
      screen.getByRole("button", { name: /save report server settings/i }),
    );

    await waitFor(() => {
      expect(updateProfileMock).toHaveBeenCalledWith(
        expect.objectContaining({
          preferencesJson: expect.stringContaining("https://hub.example.org"),
        }),
      );
    });
    const savedPrefs = JSON.parse(
      updateProfileMock.mock.calls[0][0].preferencesJson,
    );
    expect(savedPrefs.reportServerToken).toBe("tok123");
  });

  it("submitting to an unreachable org server shows the non-blocking saved-locally copy, never a modal", async () => {
    getOrCreateProfileMock.mockResolvedValue(
      baseProfile({
        preferencesJson: JSON.stringify({
          reportServerUrl: "https://unreachable.example.org",
          reportServerToken: "tok",
        }),
      }),
    );
    submitEvidenceReportMock.mockResolvedValue({ accepted: false });

    render(<SettingsReportServerSection />);
    await waitFor(() => {
      expect(screen.getByLabelText(/report server url/i)).toHaveValue(
        "https://unreachable.example.org",
      );
    });

    await userEvent.click(
      screen.getByRole("button", { name: /submit to org server/i }),
    );

    await waitFor(() => {
      expect(
        screen.getByText(/saved locally.*retry automatically/i),
      ).toBeInTheDocument();
    });
    // Never a modal — no dialog role present.
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("submitting with no URL configured shows the no-url copy", async () => {
    render(<SettingsReportServerSection />);
    await waitFor(() => {
      expect(getOrCreateProfileMock).toHaveBeenCalled();
    });

    await userEvent.click(
      screen.getByRole("button", { name: /submit to org server/i }),
    );

    await waitFor(() => {
      expect(
        screen.getByText(/add a report server url in settings/i),
      ).toBeInTheDocument();
    });
    expect(submitEvidenceReportMock).not.toHaveBeenCalled();
  });
});
