// Phase 08.2 (Cert Simplification) — CertificationProgress tests.
//
// Updated for the new model (D-20):
//   - 4-segment progress bar (instead of 3 rows)
//   - Milestone markers at 25/50/75 (earned vs locked icon state)
//   - Completion certificate badge + Download PDF button at 100%
//
// Mocking strategy: only `@/lib/tauri-commands` is mocked. The real
// useAchievementsStore (Zustand) is driven via setState so the
// component's store-subscription path fires on updates.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Achievement, TrackCertifications } from "@/types/achievements";

const { getTrackCertificationsMock } = vi.hoisted(() => ({
  getTrackCertificationsMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  getTrackCertifications: getTrackCertificationsMock,
  exportCertificate: vi.fn(),
  exportBadge: vi.fn(),
  listAchievements: vi.fn().mockResolvedValue([]),
}));

import { CertificationProgress } from "@/components/achievements/CertificationProgress";
import { useAchievementsStore } from "@/stores/useAchievementsStore";

const INITIAL_STATE = useAchievementsStore.getState();

function makeCerts(overrides: Partial<TrackCertifications> = {}): TrackCertifications {
  return {
    earnedLevels: [],
    nextLevel: null,
    criteria: "",
    ...overrides,
  };
}

function makeAchievement(overrides: Partial<Achievement> = {}): Achievement {
  return {
    id: "ach-1",
    learnerId: "lnr-1",
    trackId: "track-1",
    packId: null,
    kind: "badge",
    level: "Milestone25",
    issuedAt: "2026-06-19T12:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "",
    trackTopic: "Kubernetes",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  useAchievementsStore.setState({
    ...INITIAL_STATE,
    achievements: [],
    recentCelebration: null,
    error: null,
    isLoading: false,
  });
});

describe("CertificationProgress — Phase 08.2 (Cert Simplification)", () => {
  it("loads_on_mount_with_trackId", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(getTrackCertificationsMock).toHaveBeenCalledTimes(1);
    });
    expect(getTrackCertificationsMock).toHaveBeenCalledWith({ trackId: "track-1" });
  });

  it("renders_four_segment_progress_bar", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-progress-bar")).toBeInTheDocument();
    });
    expect(screen.getByTestId("cert-progress-bar-fill")).toBeInTheDocument();
    expect(screen.getByTestId("cert-progress-tick-25")).toBeInTheDocument();
    expect(screen.getByTestId("cert-progress-tick-50")).toBeInTheDocument();
    expect(screen.getByTestId("cert-progress-tick-75")).toBeInTheDocument();
  });

  it("renders_milestone_markers_locked_by_default", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("milestone-row-Milestone25")).toBeInTheDocument();
    });
    expect(
      screen.getByTestId("milestone-icon-Milestone25-locked"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("milestone-icon-Milestone50-locked"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("milestone-icon-Milestone75-locked"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("milestone-icon-Completion-locked"),
    ).toBeInTheDocument();
  });

  it("milestone_25_shows_earned_when_milestone25_achievement_present", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    useAchievementsStore.setState({
      achievements: [makeAchievement({ id: "m25", level: "Milestone25" })],
    });
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(
        screen.getByTestId("milestone-icon-Milestone25-earned"),
      ).toBeInTheDocument();
    });
    // Higher milestones still locked.
    expect(
      screen.getByTestId("milestone-icon-Milestone50-locked"),
    ).toBeInTheDocument();
  });

  it("shows_completion_certificate_badge_when_completion_earned", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    const completionCert = makeAchievement({
      id: "cert-completion-1",
      kind: "certificate",
      level: "Completion",
      trackId: "track-1",
    });
    const exportCertSpy = vi
      .fn()
      .mockResolvedValue({ saved: true, path: "/p.pdf" });
    useAchievementsStore.setState({
      achievements: [completionCert],
      exportCertificate: exportCertSpy,
    });

    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByText(/completion certificate earned/i)).toBeInTheDocument();
    });
    expect(
      screen.getByTestId("milestone-icon-Completion-earned"),
    ).toBeInTheDocument();
    const dlBtn = screen.getByRole("button", { name: /download pdf/i });
    expect(dlBtn).toBeInTheDocument();
    await userEvent.click(dlBtn);
    expect(exportCertSpy).toHaveBeenCalledWith(completionCert);
  });

  it("does_not_show_download_pdf_when_only_milestones_earned", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    useAchievementsStore.setState({
      achievements: [
        makeAchievement({ id: "m25", level: "Milestone25" }),
        makeAchievement({ id: "m50", level: "Milestone50" }),
        makeAchievement({ id: "m75", level: "Milestone75" }),
      ],
    });
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(
        screen.getByTestId("milestone-icon-Milestone75-earned"),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText(/completion certificate earned/i)).toBeNull();
    expect(screen.queryByRole("button", { name: /download pdf/i })).toBeNull();
  });

  it("refetches_when_new_achievement_for_track_arrives", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(getTrackCertificationsMock).toHaveBeenCalledTimes(1);
    });

    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({ earnedLevels: ["Milestone25"] }),
    );
    await act(async () => {
      useAchievementsStore.setState({
        achievements: [makeAchievement({ id: "ach-2", level: "Milestone25" })],
      });
    });

    await waitFor(() => {
      expect(getTrackCertificationsMock).toHaveBeenCalledTimes(2);
    });
  });

  it("handles_ipc_error_with_graceful_fallback", async () => {
    getTrackCertificationsMock.mockRejectedValue(new Error("ipc boom"));
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-progress-error")).toBeInTheDocument();
    });
    expect(screen.getByText(/could not load certifications/i)).toBeInTheDocument();
  });

  it("no_emoji_in_rendered_output", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    useAchievementsStore.setState({
      achievements: [
        makeAchievement({ id: "m25", level: "Milestone25" }),
        makeAchievement({
          id: "cert",
          kind: "certificate",
          level: "Completion",
        }),
      ],
    });
    const { container } = render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("certification-progress")).toBeInTheDocument();
    });
    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });
});
