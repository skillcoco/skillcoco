// Phase 16 Plan 03 Task 1 — LibraryImportSection (relocated import UI, LIB-03/D-03).
//
// Relocates the SettingsCourseImportSection import logic verbatim (openFileDialog
// -> importCourse -> getEntitlementForTrack attribution, attribution failure
// caught and ignored) into a compact inline-row presentation mounted in
// Library.tsx. Attribution renders via the shared BuyerAttributionLine
// component (D-07 — extend, don't duplicate).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ImportCourseResult } from "@/types/course-io";

const { importCourseMock, openFileDialogMock, getEntitlementForTrackMock, loadTracksMock } =
  vi.hoisted(() => ({
    importCourseMock: vi.fn(),
    openFileDialogMock: vi.fn(),
    getEntitlementForTrackMock: vi.fn().mockResolvedValue(null),
    loadTracksMock: vi.fn().mockResolvedValue(undefined),
  }));

vi.mock("@/lib/tauri-commands", () => ({
  importCourse: importCourseMock,
  openFileDialog: openFileDialogMock,
  getEntitlementForTrack: getEntitlementForTrackMock,
}));

// Selector-aware store mock (mirrors Library.test.tsx's precedent).
vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector?: (s: { loadTracks: typeof loadTracksMock }) => unknown) => {
    const state = { loadTracks: loadTracksMock };
    return typeof selector === "function" ? selector(state) : state;
  }),
}));

import { LibraryImportSection } from "@/components/library/LibraryImportSection";

function importResult(overrides: Partial<ImportCourseResult> = {}): ImportCourseResult {
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
  loadTracksMock.mockResolvedValue(undefined);
});

describe("LibraryImportSection — Phase 16 Plan 03 Task 1", () => {
  it("shows the compact 'Import course file' inline row", () => {
    render(<LibraryImportSection />);
    expect(screen.getByTestId("import-course-button")).toBeInTheDocument();
    expect(screen.getByText("Import course file")).toBeInTheDocument();
  });

  it("imports successfully and shows attribution via BuyerAttributionLine", async () => {
    importCourseMock.mockResolvedValue(importResult());
    getEntitlementForTrackMock.mockResolvedValue({
      issuerName: "Test Publisher",
      buyerName: "Jane Buyer",
      orderId: "ORD-77",
    });

    render(<LibraryImportSection />);
    await userEvent.click(screen.getByTestId("import-course-button"));

    await waitFor(() => {
      expect(screen.getByText(/course imported successfully/i)).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("Licensed to Jane Buyer · order #ORD-77")).toBeInTheDocument();
    });
    expect(getEntitlementForTrackMock).toHaveBeenCalledWith("trk-abc123");
  });

  it("cancelling the file dialog returns to idle with no error", async () => {
    openFileDialogMock.mockResolvedValue(null);
    render(<LibraryImportSection />);
    await userEvent.click(screen.getByTestId("import-course-button"));

    await waitFor(() => {
      expect(importCourseMock).not.toHaveBeenCalled();
    });
    expect(screen.queryByText(/import failed/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/course imported successfully/i)).not.toBeInTheDocument();
  });

  // WR-03 — a successful import must refresh the "Your packs" grid on the
  // same page (useLearningStore.loadTracks), not leave it stale until the
  // user navigates away and back.
  it("refreshes the tracks slice after a successful import", async () => {
    importCourseMock.mockResolvedValue(importResult());
    render(<LibraryImportSection />);
    await userEvent.click(screen.getByTestId("import-course-button"));

    await waitFor(() => {
      expect(screen.getByText(/course imported successfully/i)).toBeInTheDocument();
    });
    expect(loadTracksMock).toHaveBeenCalled();
  });

  it("does not refresh the tracks slice when import fails", async () => {
    importCourseMock.mockRejectedValue(new Error("bad pack"));
    render(<LibraryImportSection />);
    await userEvent.click(screen.getByTestId("import-course-button"));

    await waitFor(() => {
      expect(screen.getByText(/import failed/i)).toBeInTheDocument();
    });
    expect(loadTracksMock).not.toHaveBeenCalled();
  });

  it("shows the inline error block (never a raw error string) on import failure", async () => {
    importCourseMock.mockRejectedValue(new Error("bad pack"));
    render(<LibraryImportSection />);
    await userEvent.click(screen.getByTestId("import-course-button"));

    await waitFor(() => {
      expect(screen.getByText(/import failed/i)).toBeInTheDocument();
    });
  });
});
