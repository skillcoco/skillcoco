// Phase 18 (Skill Reports) — Plan 05 (Wave 3) Zustand slice.
//
// SIBLING slice (NOT an extension of useLearningStore/useAchievementsStore)
// per the sibling-slice rule (Phase 4 Pitfall 5 + Phase 03.1 useLabStore +
// 06-04 useAchievementsStore precedent). Grep guard:
// `rg "useLearningStore|useAchievementsStore" src/stores/useReportsStore.ts`
// must return 0 code references.
//
// Mirrors useAchievementsStore's exportCertificate/exportBadge bytes-in-hand
// + native-save-dialog flow (Wave 2 IPC pattern), adapted to the REP-01
// one-action dual PDF+JSON skill-report export. The filename convention is
// `learnforge-skill-report-{slug}-{YYYYMMDD}.{pdf|json}` where slug is
// derived from the track topic (per-track scope) or "profile"
// (whole-profile scope) — Claude's Discretion per 18-CONTEXT.md.

import { create } from "zustand";
import {
  exportReportPdf as exportReportPdfCmd,
  exportReportJson as exportReportJsonCmd,
} from "@/lib/tauri-commands";

export interface ExportReportResult {
  saved: boolean;
  path: string | null;
}

export interface ExportReportParams {
  /// "track" | "whole-profile" — mirrors the Rust ReportScope wire shape.
  scope: "track" | "whole-profile";
  trackId?: string;
  /// Used only to derive the filename slug; not sent over IPC.
  trackTopic?: string;
  /// D-10 confirm-at-export learner name, baked into the signed payload.
  learnerName: string;
}

interface ReportsState {
  isExporting: boolean;
  error: string | null;

  exportReportPdf: (params: ExportReportParams) => Promise<ExportReportResult>;
  exportReportJson: (params: ExportReportParams) => Promise<ExportReportResult>;
}

const INITIAL: Omit<ReportsState, "exportReportPdf" | "exportReportJson"> = {
  isExporting: false,
  error: null,
};

// ── helpers ─────────────────────────────────────────────────────────

/// Mirrors useAchievementsStore's T-06-14 mitigation: strip everything that
/// is not `[a-z0-9-]` so the suggested filename is always safe to pass to
/// the dialog plugin. The user still confirms the final path interactively.
function slugify(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function isoToCompactDate(iso: string): string {
  // "2026-06-16T12:34:56Z" -> "20260616"
  return iso.slice(0, 10).replace(/-/g, "");
}

function suggestedFilename(
  params: ExportReportParams,
  ext: "pdf" | "json",
): string {
  const slug = slugify(params.trackTopic || "profile");
  const date = isoToCompactDate(new Date().toISOString());
  return `learnforge-skill-report-${slug}-${date}.${ext}`;
}

// ── store ───────────────────────────────────────────────────────────

export const useReportsStore = create<ReportsState>((set) => ({
  ...INITIAL,

  exportReportPdf: async (params) => {
    set({ isExporting: true, error: null });
    try {
      const filename = suggestedFilename(params, "pdf");
      const path = await exportReportPdfCmd(
        {
          scope: params.scope,
          trackId: params.trackId,
          learnerName: params.learnerName,
        },
        filename,
      );
      set({ isExporting: false });
      return { saved: path !== null, path };
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      set({ isExporting: false, error: message });
      throw err;
    }
  },

  exportReportJson: async (params) => {
    set({ isExporting: true, error: null });
    try {
      const filename = suggestedFilename(params, "json");
      const path = await exportReportJsonCmd(
        {
          scope: params.scope,
          trackId: params.trackId,
          learnerName: params.learnerName,
        },
        filename,
      );
      set({ isExporting: false });
      return { saved: path !== null, path };
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      set({ isExporting: false, error: message });
      throw err;
    }
  },
}));

/// Test-only helper — resets the store to its initial state.
/// Mirrors `useAchievementsStore.__resetStore`.
export function __resetStore(): void {
  useReportsStore.setState({ ...INITIAL });
}
