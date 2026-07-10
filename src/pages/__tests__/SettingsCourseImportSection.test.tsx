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

const { importCourseMock, openFileDialogMock } = vi.hoisted(() => ({
  importCourseMock: vi.fn(),
  openFileDialogMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  importCourse: importCourseMock,
  openFileDialog: openFileDialogMock,
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
