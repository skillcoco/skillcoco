import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// ── Mock Tauri IPC ─────────────────────────────────────────────────────────────
// Must be hoisted before the component import so the mock is in place when
// the module initialises. Uses vi.hoisted for the function references so they
// can be reassigned per-test.

const mockGetLessonVideos = vi.hoisted(() => vi.fn());
const mockRefreshLessonVideos = vi.hoisted(() => vi.fn());

vi.mock("@/lib/tauri-commands", () => ({
  getLessonVideos: mockGetLessonVideos,
  refreshLessonVideos: mockRefreshLessonVideos,
}));

import { ReferenceVideoPanel } from "../ReferenceVideoPanel";
import type { LessonVideosResult } from "@/types/videos";

// ── Helpers ────────────────────────────────────────────────────────────────────

const VIDEO_A = {
  videoId: "vidA",
  title: "Pod Lifecycle Explained",
  channelTitle: "KubeLearner",
  relevanceScore: 0.85,
};

function makeResult(videos: typeof VIDEO_A[]): LessonVideosResult {
  return { videos };
}

const DEFAULT_PROPS = {
  moduleId: "mod-1",
  sectionId: "sec-1",
  sectionTitle: "Pod Lifecycle",
};

// ── Tests ──────────────────────────────────────────────────────────────────────

describe("ReferenceVideoPanel (acceptance)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── D-09: null-on-empty ────────────────────────────────────────────────────

  it("ref_vid_null_on_empty — renders nothing when backend returns empty list", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    const { container } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledTimes(1);
    });

    // D-09: no element should be in the DOM
    expect(container.firstChild).toBeNull();
    expect(screen.queryByTestId("reference-video-panel")).toBeNull();
  });

  it("ref_vid_null_on_error — renders nothing when IPC call throws", async () => {
    mockGetLessonVideos.mockRejectedValue(new Error("quota exceeded"));

    const { container } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledTimes(1);
    });

    expect(container.firstChild).toBeNull();
  });

  // ── Single video render ────────────────────────────────────────────────────

  it("ref_vid_renders_single_video — panel shows when backend returns one video", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    // Title and channel must be visible
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    expect(screen.getByText(VIDEO_A.channelTitle)).toBeInTheDocument();
  });

  it("ref_vid_only_first_video — takes first video when backend (incorrectly) returns multiple", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Another Pod Video" };
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A, VIDEO_B]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    // Only the first video's title should appear
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    expect(screen.queryByText(VIDEO_B.title)).not.toBeInTheDocument();
  });

  // ── "Reference" / "optional" framing copy ────────────────────────────────

  it("ref_vid_reference_heading — renders 'Reference video' heading", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText("Reference video")).toBeInTheDocument();
    });
  });

  it("ref_vid_optional_framing — renders optional/supplementary framing copy", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      // The framing copy must mention "Optional" and "primary"
      const panel = screen.getByTestId("reference-video-panel");
      expect(panel.textContent).toMatch(/optional/i);
      expect(panel.textContent).toMatch(/primary/i);
    });
  });

  // ── youtube-nocookie embed (D-07) ─────────────────────────────────────────

  it("ref_vid_nocookie_embed — iframe src uses youtube-nocookie.com domain", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      const iframe = screen.getByTitle(VIDEO_A.title) as HTMLIFrameElement;
      expect(iframe.src).toContain("youtube-nocookie.com");
      expect(iframe.src).toContain(VIDEO_A.videoId);
    });
  });

  // ── Per-section re-fetch on sectionId change ───────────────────────────────

  it("ref_vid_refetch_on_section_change — calls getLessonVideos again when sectionId changes", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Service Mesh Deep Dive" };
    mockGetLessonVideos
      .mockResolvedValueOnce(makeResult([VIDEO_A])) // sec-1
      .mockResolvedValueOnce(makeResult([VIDEO_B])); // sec-2

    const { rerender } = render(
      <ReferenceVideoPanel moduleId="mod-1" sectionId="sec-1" sectionTitle="Pod Lifecycle" />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    // Simulate Next-lesson click — sectionId changes
    rerender(
      <ReferenceVideoPanel
        moduleId="mod-1"
        sectionId="sec-2"
        sectionTitle="Service Mesh"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });

    expect(mockGetLessonVideos).toHaveBeenCalledTimes(2);
    expect(mockGetLessonVideos).toHaveBeenNthCalledWith(1, "mod-1", "sec-1", "Pod Lifecycle");
    expect(mockGetLessonVideos).toHaveBeenNthCalledWith(2, "mod-1", "sec-2", "Service Mesh");
  });

  it("ref_vid_section_passes_correct_args — passes moduleId, sectionId, sectionTitle to IPC", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    render(
      <ReferenceVideoPanel
        moduleId="mod-xyz"
        sectionId="sec-abc"
        sectionTitle="Introduction to Pods"
      />,
    );

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledWith(
        "mod-xyz",
        "sec-abc",
        "Introduction to Pods",
      );
    });
  });

  // ── Refresh button ────────────────────────────────────────────────────────

  it("ref_vid_refresh_button_calls_refresh_with_section_args", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    const refreshedVideo = { ...VIDEO_A, videoId: "vidNew", title: "New Reference" };
    mockRefreshLessonVideos.mockResolvedValue(makeResult([refreshedVideo]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    const refreshBtn = screen.getByRole("button", { name: /refresh reference video/i });
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(mockRefreshLessonVideos).toHaveBeenCalledWith(
        DEFAULT_PROPS.moduleId,
        DEFAULT_PROPS.sectionId,
        DEFAULT_PROPS.sectionTitle,
      );
    });

    await waitFor(() => {
      expect(screen.getByText(refreshedVideo.title)).toBeInTheDocument();
    });
  });

  it("ref_vid_refresh_null_on_empty — hides panel when refresh returns empty", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    const refreshBtn = screen.getByRole("button", { name: /refresh reference video/i });
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(screen.queryByTestId("reference-video-panel")).toBeNull();
    });
  });

  // ── data-testid present ───────────────────────────────────────────────────

  it("ref_vid_has_testid — panel has data-testid='reference-video-panel'", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });
  });
});
