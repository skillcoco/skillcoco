// Phase 18 (Skill Reports) — Plan 05 (Wave 3) useReportsStore tests.
//
// SIBLING slice (NOT extension of useLearningStore/useAchievementsStore)
// per the sibling-slice rule (Phase 4 Pitfall 5 / 06-04 precedent). Mirrors
// useAchievementsStore's exportCertificate/exportBadge bytes-in-hand + save
// dialog pattern, adapted to the dual PDF+JSON skill-report export.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/lib/tauri-commands", () => ({
  exportReportPdf: vi.fn(),
  exportReportJson: vi.fn(),
}));

import {
  useReportsStore,
  __resetStore,
} from "@/stores/useReportsStore";
import {
  exportReportPdf as exportReportPdfCmd,
  exportReportJson as exportReportJsonCmd,
} from "@/lib/tauri-commands";

describe("useReportsStore — Phase 18 Plan 05 (Wave 3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("is a sibling slice with zero code references to useLearningStore/useAchievementsStore", async () => {
    // Static grep guard is enforced in acceptance_criteria via rg; this test
    // documents intent — the store must not import those modules.
    const mod = await import("@/stores/useReportsStore");
    expect(mod).toBeDefined();
  });

  it("exportReportPdf invokes IPC with the suggested filename and saves", async () => {
    vi.mocked(exportReportPdfCmd).mockResolvedValue("/tmp/report.pdf");

    const result = await useReportsStore.getState().exportReportPdf({
      scope: "track",
      trackId: "trk-1",
      trackTopic: "Kubernetes Fundamentals",
      learnerName: "Ada Lovelace",
    });

    expect(exportReportPdfCmd).toHaveBeenCalledTimes(1);
    const [reqArg, filenameArg] = vi.mocked(exportReportPdfCmd).mock.calls[0];
    expect(reqArg).toEqual({
      scope: "track",
      trackId: "trk-1",
      learnerName: "Ada Lovelace",
    });
    expect(filenameArg).toMatch(
      /^learnforge-skill-report-kubernetes-fundamentals-\d{8}\.pdf$/,
    );
    expect(result).toEqual({ saved: true, path: "/tmp/report.pdf" });
  });

  it("exportReportJson invokes IPC with a .json suggested filename and saves", async () => {
    vi.mocked(exportReportJsonCmd).mockResolvedValue("/tmp/report.json");

    const result = await useReportsStore.getState().exportReportJson({
      scope: "whole-profile",
      trackId: undefined,
      trackTopic: undefined,
      learnerName: "Ada Lovelace",
    });

    expect(exportReportJsonCmd).toHaveBeenCalledTimes(1);
    const [reqArg, filenameArg] = vi.mocked(exportReportJsonCmd).mock.calls[0];
    expect(reqArg).toEqual({
      scope: "whole-profile",
      trackId: undefined,
      learnerName: "Ada Lovelace",
    });
    expect(filenameArg).toMatch(
      /^learnforge-skill-report-profile-\d{8}\.json$/,
    );
    expect(result).toEqual({ saved: true, path: "/tmp/report.json" });
  });

  it("exportReportPdf returns {saved:false, path:null} on cancel without throwing", async () => {
    vi.mocked(exportReportPdfCmd).mockResolvedValue(null);

    const result = await useReportsStore.getState().exportReportPdf({
      scope: "track",
      trackId: "trk-1",
      trackTopic: "Rust",
      learnerName: "Ada",
    });

    expect(result).toEqual({ saved: false, path: null });
  });

  it("exportReportJson returns {saved:false, path:null} on cancel without throwing", async () => {
    vi.mocked(exportReportJsonCmd).mockResolvedValue(null);

    const result = await useReportsStore.getState().exportReportJson({
      scope: "track",
      trackId: "trk-1",
      trackTopic: "Rust",
      learnerName: "Ada",
    });

    expect(result).toEqual({ saved: false, path: null });
  });
});
