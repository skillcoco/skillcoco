// Phase 14 Plan 06 (14-06, CR-01) — SettingsCourseImportSection tests.
//
// The review flagged that `handleImport`'s success setState copies only
// trackId/moduleCount/blockCount/warnings from `importCourse`'s result,
// silently discarding `result.verified` / `result.issuerName` — the two
// fields the Step 3.5 crypto verification gate surfaces for the frontend
// "verified licensor" badge (D-14). This test drives a successful import
// through the component and asserts the post-import state actually
// carries verified/issuerName instead of dropping them.
//
// Mocking strategy (mirrors SettingsVerifyCertSection.test.tsx): only
// `@/lib/tauri-commands` is mocked via vi.hoisted so the mock references
// survive vi.mock factory hoisting.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ImportCourseResult } from "@/types/course-io";

const { importCourseMock, openFileDialogMock, getEntitlementForTrackMock } = vi.hoisted(() => ({
  importCourseMock: vi.fn(),
  openFileDialogMock: vi.fn(),
  // Phase 15 Plan 06 (D-08) — default: no entitlement (most imports are
  // unlicensed, e.g. AI-generated/exported courses re-imported elsewhere).
  getEntitlementForTrackMock: vi.fn().mockResolvedValue(null),
}));

vi.mock("@/lib/tauri-commands", () => ({
  importCourse: importCourseMock,
  openFileDialog: openFileDialogMock,
  getEntitlementForTrack: getEntitlementForTrackMock,
}));

import { SettingsCourseImportSection } from "@/pages/SettingsCourseImportSection";

function verifiedResult(
  overrides: Partial<ImportCourseResult> = {},
): ImportCourseResult {
  return {
    trackId: "trk-abc123",
    moduleCount: 4,
    blockCount: 12,
    warnings: [],
    verified: true,
    issuerName: "Test Publisher",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  openFileDialogMock.mockResolvedValue("/tmp/course.json");
  getEntitlementForTrackMock.mockResolvedValue(null);
});

describe("SettingsCourseImportSection — Phase 14 Plan 06 (14-06 CR-01)", () => {
  it("surfaces result.verified and result.issuerName in post-import state instead of discarding them", async () => {
    importCourseMock.mockResolvedValue(verifiedResult());

    render(<SettingsCourseImportSection />);

    const button = screen.getByTestId("import-course-button");
    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/course imported successfully/i)).toBeInTheDocument();
    });

    // The component must expose the verified issuer somewhere in its
    // rendered output once the fields are copied into state — assert via
    // the track id / summary render as a proxy that the success branch ran,
    // then assert the underlying state carried verified/issuerName by
    // checking the DOM for the issuer name text (component renders it once
    // ImportState.issuerName is populated).
    expect(screen.getByText(/test publisher/i)).toBeInTheDocument();
  });

  it("does not render a verified/issuer indicator for an unsigned import (fields undefined)", async () => {
    importCourseMock.mockResolvedValue(
      verifiedResult({ verified: undefined, issuerName: undefined }),
    );

    render(<SettingsCourseImportSection />);

    const button = screen.getByTestId("import-course-button");
    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/course imported successfully/i)).toBeInTheDocument();
    });

    expect(screen.queryByText(/test publisher/i)).not.toBeInTheDocument();
  });
});

describe("SettingsCourseImportSection buyer attribution (15-06, D-08)", () => {
  it("renders 'Licensed to {buyer} · order #{id}' when an entitlement exists for the imported track", async () => {
    importCourseMock.mockResolvedValue(verifiedResult());
    getEntitlementForTrackMock.mockResolvedValue({
      issuerName: "Test Publisher",
      buyerName: "Jane Buyer",
      orderId: "ORD-77",
    });

    render(<SettingsCourseImportSection />);

    const button = screen.getByTestId("import-course-button");
    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText("Licensed to Jane Buyer · order #ORD-77")).toBeInTheDocument();
    });
    expect(getEntitlementForTrackMock).toHaveBeenCalledWith("trk-abc123");
  });

  it("omits the attribution line entirely when no entitlement exists for the imported track", async () => {
    importCourseMock.mockResolvedValue(verifiedResult());
    getEntitlementForTrackMock.mockResolvedValue(null);

    render(<SettingsCourseImportSection />);

    const button = screen.getByTestId("import-course-button");
    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/course imported successfully/i)).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(getEntitlementForTrackMock).toHaveBeenCalledWith("trk-abc123");
    });
    expect(screen.queryByText(/Licensed to/)).not.toBeInTheDocument();
  });
});
