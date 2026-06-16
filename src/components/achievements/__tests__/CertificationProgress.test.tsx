// Phase 6 (Certification) — Plan 06-05 (Wave 4) CertificationProgress tests.
//
// Per D-11 + CERT-11: three-row progress indicator on TrackView. Reads
// `getTrackCertifications(trackId)` once on mount, re-fetches when newly
// issued achievements arrive in the store for the same track, and shows
// a Completion-certificate download link when Professional is earned.
// No emojis (D-08); lucide icons only.
//
// Mocking strategy: only `@/lib/tauri-commands` is mocked. The real
// useAchievementsStore (a Zustand store) is used directly via setState
// so the selector subscription in the component fires on store updates.
// The store's exportCertificate is replaced by a spy via setState for the
// completion-cert download assertion.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Achievement, TrackCertifications } from "@/types/achievements";

// ── Mocks ────────────────────────────────────────────────────────────

const { getTrackCertificationsMock } = vi.hoisted(() => ({
  getTrackCertificationsMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  getTrackCertifications: getTrackCertificationsMock,
  // exportCertificate / exportBadge are reached only via the real store
  // for the download-PDF test. We swap exportCertificate via setState
  // instead — see the test below.
  exportCertificate: vi.fn(),
  exportBadge: vi.fn(),
  listAchievements: vi.fn().mockResolvedValue([]),
}));

import { CertificationProgress } from "@/components/achievements/CertificationProgress";
import { useAchievementsStore } from "@/stores/useAchievementsStore";

// Snapshot the initial state so each test gets a clean store.
const INITIAL_STATE = useAchievementsStore.getState();

function makeCerts(overrides: Partial<TrackCertifications> = {}): TrackCertifications {
  return {
    earnedLevels: [],
    nextLevel: "Associate",
    criteria: "Master 25% of modules",
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
    level: "Associate",
    issuedAt: "2026-06-16T12:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "deadbeef",
    trackTopic: "Kubernetes",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  // Reset the real store between tests.
  useAchievementsStore.setState({
    ...INITIAL_STATE,
    achievements: [],
    recentCelebration: null,
    error: null,
    isLoading: false,
  });
});

describe("CertificationProgress — Phase 6 Plan 06-05 (Wave 4)", () => {
  it("loads_on_mount_with_trackId", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(getTrackCertificationsMock).toHaveBeenCalledTimes(1);
    });
    expect(getTrackCertificationsMock).toHaveBeenCalledWith({ trackId: "track-1" });
  });

  it("renders_three_level_rows", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-row-Associate")).toBeInTheDocument();
    });
    expect(screen.getByTestId("cert-row-Practitioner")).toBeInTheDocument();
    expect(screen.getByTestId("cert-row-Professional")).toBeInTheDocument();
  });

  it("earned_rows_show_check_icon", async () => {
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate"],
        nextLevel: "Practitioner",
        criteria: "Master 60% of modules",
      }),
    );
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-row-Associate-icon-check")).toBeInTheDocument();
    });
  });

  it("next_level_shows_in_progress_icon_and_criteria", async () => {
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate"],
        nextLevel: "Practitioner",
        criteria: "Master 60% of modules",
      }),
    );
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-row-Practitioner-icon-progress")).toBeInTheDocument();
    });
    expect(screen.getByText(/master 60%/i)).toBeInTheDocument();
  });

  it("future_levels_show_lock_icon", async () => {
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate"],
        nextLevel: "Practitioner",
        criteria: "Master 60% of modules",
      }),
    );
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("cert-row-Professional-icon-lock")).toBeInTheDocument();
    });
  });

  it("no_emoji_in_rendered_output", async () => {
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate", "Practitioner"],
        nextLevel: "Professional",
        criteria: "Master 100% of modules + 0.85 average mastery",
      }),
    );
    const { container } = render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByTestId("certification-progress")).toBeInTheDocument();
    });
    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });

  it("shows_completion_certificate_note_when_professional_earned", async () => {
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate", "Practitioner", "Professional"],
        nextLevel: null,
        criteria: "",
      }),
    );
    const completionCert = makeAchievement({
      id: "cert-completion-1",
      kind: "certificate",
      level: "Completion",
      trackId: "track-1",
    });
    const exportCertSpy = vi
      .fn()
      .mockResolvedValue({ saved: true, path: "/p.pdf" });
    // Pre-populate the store with the completion cert achievement, and
    // swap exportCertificate with a spy so we can assert the click wires
    // through to the store action.
    useAchievementsStore.setState({
      achievements: [completionCert],
      exportCertificate: exportCertSpy,
    });

    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(screen.getByText(/completion certificate earned/i)).toBeInTheDocument();
    });
    const dlBtn = screen.getByRole("button", { name: /download pdf/i });
    expect(dlBtn).toBeInTheDocument();
    await userEvent.click(dlBtn);
    expect(exportCertSpy).toHaveBeenCalledWith(completionCert);
  });

  it("refetches_when_new_achievement_for_track_arrives", async () => {
    getTrackCertificationsMock.mockResolvedValue(makeCerts());
    render(<CertificationProgress trackId="track-1" />);

    await waitFor(() => {
      expect(getTrackCertificationsMock).toHaveBeenCalledTimes(1);
    });

    // Newly issued achievement for the same track arrives in the store —
    // component should re-fetch.
    getTrackCertificationsMock.mockResolvedValue(
      makeCerts({
        earnedLevels: ["Associate"],
        nextLevel: "Practitioner",
        criteria: "Master 60% of modules",
      }),
    );
    await act(async () => {
      useAchievementsStore.setState({
        achievements: [makeAchievement({ id: "ach-2", trackId: "track-1" })],
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
});
